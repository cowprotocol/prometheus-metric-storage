//! This crate is an implementation detail for [`prometheus-metric-storage`][1].
//!
//! [1]: https://crates.io/crates/prometheus-metric-storage

#![deny(unsafe_code)]

use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned, ToTokens};
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, Data, DeriveInput, Error, Field, Fields, Index, Lit, Meta, MetaList,
    NestedMeta, Result,
};

#[doc(hidden)]
#[proc_macro_derive(MetricStorage, attributes(metric))]
pub fn metric_storage(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    expand(input)
        .unwrap_or_else(|err| err.to_compile_error())
        .into()
}

fn expand(input: DeriveInput) -> Result<TokenStream> {
    let name = input.ident;

    let attrs = MetricAttrs::parse(&input.attrs, true)?;

    let input = match input.data {
        Data::Struct(input) => input,
        Data::Enum(_) => panic!("MetricsStorage can't be implemented for enums"),
        Data::Union(_) => panic!("MetricsStorage can't be implemented for unions"),
    };

    let subsystem = attrs.subsystem.unwrap_or_else(|| "".to_string());

    let labels = attrs.labels.unwrap_or_default();
    let label_idents: Vec<_> = labels
        .iter()
        .map(|l| Ident::new(l, Span::call_site()))
        .collect();

    let (init, reg) = match input.fields {
        Fields::Named(fields) => {
            let ident: Vec<_> = fields
                .named
                .iter()
                .map(|field| field.ident.clone().unwrap())
                .collect();
            let reg = registrators(
                fields
                    .named
                    .iter()
                    .map(|field| field.ident.clone().unwrap()),
            );
            let init = initializers(fields.named.into_iter(), subsystem)?;
            let init = quote! { Self { #(#ident: #init,)* } };
            (init, reg)
        }
        Fields::Unnamed(fields) => {
            let reg = registrators((0..fields.unnamed.len()).map(|i| Index {
                index: i as _,
                span: Span::call_site(),
            }));
            let init = initializers(fields.unnamed.into_iter(), subsystem)?;
            let init = quote! { Self ( #(#init,)* ) };
            (init, reg)
        }
        Fields::Unit => (quote! { Self }, quote! {}),
    };

    Ok(quote! {
        #[allow(
            clippy::vec_init_then_push,
            clippy::redundant_clone,
            clippy::let_and_return,
            unused,
            unused_mut
        )]
        impl prometheus_metric_storage::MetricStorage for #name {
            fn const_labels() -> &'static [&'static str] {
                &[#(#labels,)*]
            }

            fn from_const_labels_unregistered(
                const_labels: std::collections::HashMap<String, String>
            ) -> prometheus_metric_storage::Result<Self> {
                Ok(#init)
            }

            fn register(
                &self, registry: &prometheus_metric_storage::Registry
            ) -> prometheus_metric_storage::Result<()> {
                #reg
                Ok(())
            }
        }

        #[allow(
            clippy::vec_init_then_push,
            clippy::redundant_clone,
            clippy::let_and_return,
            unused,
            unused_mut
        )]
        impl #name {
            fn new_unregistered(
                #(#label_idents: String,)*
            ) -> prometheus_metric_storage::Result<Self> {
                let mut const_labels = std::collections::HashMap::new();
                #(const_labels.insert(#labels.to_string(), #label_idents);)*

                <Self as prometheus_metric_storage::MetricStorage>::from_const_labels_unregistered(const_labels)
            }

            fn new(
                registry: &prometheus_metric_storage::Registry, #(#label_idents: String,)*
            ) -> prometheus_metric_storage::Result<Self> {
                let metrics = Self::new_unregistered(#(#label_idents,)*)?;
                <Self as prometheus_metric_storage::MetricStorage>::register(&metrics, registry)?;
                Ok(metrics)
            }

            fn instance(
                registry: &prometheus_metric_storage::StorageRegistry, #(#label_idents: String,)*
            ) -> prometheus_metric_storage::Result<Self> {
                let mut const_labels = std::collections::HashMap::new();
                #(const_labels.insert(#labels.to_string(), #label_idents);)*

                registry.get_or_create_storage::<Self>(const_labels)
            }
        }
    })
}

fn initializers(
    fields: impl Iterator<Item = Field>,
    subsystem: String,
) -> Result<Vec<TokenStream>> {
    fields
        .map(|field| {
            let MetricAttrs {
                name,
                help,
                labels,
                buckets,
                ..
            } = MetricAttrs::parse(&field.attrs, false)?;

            let name = name.or_else(|| field.ident.as_ref().map(|ident| ident.to_string()));
            let name = match name {
                Some(name) if !name.is_empty() => name,
                _ => {
                    return Err(Error::new(
                        field.span(),
                        "metric name is required, consider adding `#[metric(name = \"...\")]`",
                    ))
                }
            };

            let help = match help {
                Some(help) if !help.is_empty() => help,
                _ => {
                    return Err(Error::new(
                        field.span(),
                        "metric help message is required, consider adding a docstring",
                    ))
                }
            };

            let labels = labels.unwrap_or_default();

            let opts = quote_spanned! { field.span() =>
                prometheus_metric_storage::Opts {
                    namespace: "".to_string(),
                    subsystem: #subsystem.to_string(),
                    name: #name.to_string(),
                    help: #help.to_string(),
                    const_labels: const_labels.clone(),
                    variable_labels: {
                        let mut labels = Vec::new();
                        #(labels.push(#labels.to_string());)*
                        labels
                    }
                }
            };

            if let Some(buckets) = buckets {
                Ok(quote_spanned! { field.span() =>
                    prometheus_metric_storage::HistMetricInit::init(
                        #opts,
                        {
                            let mut buckets = Vec::new();
                            #(buckets.push(#buckets);)*
                            buckets
                        }
                    )?
                })
            } else {
                Ok(quote! {
                    prometheus_metric_storage::MetricInit::init(#opts)?
                })
            }
        })
        .collect()
}

fn registrators<I: Iterator<Item = T>, T: ToTokens>(ident: I) -> TokenStream {
    quote! { #(registry.register(Box::new(self.#ident.clone()))?;)* }
}

#[derive(Default, Debug)]
struct MetricAttrs {
    subsystem: Option<String>,
    name: Option<String>,
    help: Option<String>,
    labels: Option<Vec<String>>,
    buckets: Option<Vec<f64>>,
}

impl MetricAttrs {
    fn parse(attrs: &[syn::Attribute], is_struct_level: bool) -> Result<Self> {
        let mut result = Self::default();

        let mut doc = None;

        for attr in attrs {
            if attr.path.is_ident("metric") {
                let list = match attr.parse_meta()? {
                    Meta::List(list) => list,
                    _ => {
                        return Err(Error::new(
                            attr.path.span(),
                            "value for the `metric` attribute should be a list: `metric(...)`",
                        ))
                    }
                };

                for attr in list.nested {
                    let attr = match attr {
                        NestedMeta::Meta(attr) => attr,
                        NestedMeta::Lit(lit) => {
                            return Err(Error::new(lit.span(), "expected a named parameter"))
                        }
                    };

                    let path = attr.path();
                    if is_struct_level && path.is_ident("subsystem") {
                        result.parse_subsystem(attr)?
                    } else if !is_struct_level && path.is_ident("name") {
                        result.parse_name(attr)?
                    } else if !is_struct_level && path.is_ident("help") {
                        result.parse_help(attr)?
                    } else if path.is_ident("labels") {
                        result.parse_labels(attr)?
                    } else if !is_struct_level && path.is_ident("buckets") {
                        result.parse_buckets(attr)?
                    } else {
                        return Err(Error::new(path.span(), "unexpected parameter"));
                    }
                }
            } else if doc.is_none() && attr.path.is_ident("doc") {
                doc = match attr.parse_meta()? {
                    Meta::NameValue(kv) => match kv.lit {
                        Lit::Str(s) => Some(s.value().trim().to_string()),
                        _ => None,
                    },
                    _ => None,
                };
            }
        }

        if result.help.is_none() {
            result.help = doc;
        }

        Ok(result)
    }

    fn parse_subsystem(&mut self, meta: Meta) -> Result<()> {
        Self::check_none("subsystem", meta.path().span(), self.subsystem.is_some())?;

        self.subsystem = Some(Self::value_to_string(Self::meta_to_value(meta)?)?);

        Ok(())
    }

    fn parse_name(&mut self, meta: Meta) -> Result<()> {
        Self::check_none("name", meta.path().span(), self.name.is_some())?;

        self.name = Some(Self::value_to_string(Self::meta_to_value(meta)?)?);

        Ok(())
    }

    fn parse_help(&mut self, meta: Meta) -> Result<()> {
        Self::check_none("help", meta.path().span(), self.help.is_some())?;

        self.help = Some(Self::value_to_string(Self::meta_to_value(meta)?)?);

        Ok(())
    }

    fn parse_labels(&mut self, meta: Meta) -> Result<()> {
        Self::check_none("labels", meta.path().span(), self.labels.is_some())?;

        let mut labels = Vec::new();
        for label in Self::meta_to_list(meta)?.nested {
            let label_span = label.span();
            let value = Self::value_to_string(Self::nested_meta_to_value(label)?)?;
            if labels.contains(&value) {
                return Err(Error::new(label_span, "duplicate label"));
            }
            labels.push(value)
        }
        self.labels = Some(labels);

        Ok(())
    }

    fn parse_buckets(&mut self, meta: Meta) -> Result<()> {
        Self::check_none("buckets", meta.path().span(), self.buckets.is_some())?;

        let mut buckets = Vec::new();
        for label in Self::meta_to_list(meta)?.nested {
            buckets.push(Self::value_to_float(Self::nested_meta_to_value(label)?)?)
        }
        self.buckets = Some(buckets);

        Ok(())
    }

    fn meta_to_value(meta: Meta) -> Result<Lit> {
        match meta {
            Meta::NameValue(kv) => Ok(kv.lit),
            _ => Err(Error::new(meta.path().span(), "expected a value")),
        }
    }

    fn nested_meta_to_value(meta: NestedMeta) -> Result<Lit> {
        match meta {
            NestedMeta::Lit(lit) => Ok(lit),
            _ => Err(Error::new(meta.span(), "expected a value")),
        }
    }

    fn meta_to_list(meta: Meta) -> Result<MetaList> {
        match meta {
            Meta::List(list) => Ok(list),
            _ => Err(Error::new(meta.path().span(), "expected a list of values")),
        }
    }

    fn value_to_string(lit: Lit) -> Result<String> {
        match lit {
            Lit::Str(s) => Ok(s.value()),
            _ => Err(Error::new(lit.span(), "expected a string")),
        }
    }

    fn value_to_float(lit: Lit) -> Result<f64> {
        match lit {
            Lit::Int(i) => i.base10_parse(),
            Lit::Float(f) => f.base10_parse(),
            _ => Err(Error::new(lit.span(), "expected a floating point number")),
        }
    }

    fn check_none(name: &str, span: Span, is_some: bool) -> Result<()> {
        if is_some {
            Err(Error::new(span, format!("{} is redefined", name)))
        } else {
            Ok(())
        }
    }
}
