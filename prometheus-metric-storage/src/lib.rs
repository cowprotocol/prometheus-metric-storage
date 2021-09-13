//! Derive macro to instantiate and register [`prometheus`] metrics without
//! having to write tons of boilerplate code.
//!
//! # Motivation
//!
//! When instrumenting code with prometheus metrics, one is required
//! to write quite a bit of boilerplate code.
//!
//! Creating metrics, setting up their options, registering them,
//! having to store metrics in some struct and pass this struct around,
//! all of this is cumbersome to say the least.
//!
//! The situation is partially alleviated by using the [static metrics] mechanism,
//! that is, metrics defined within a `lazy_static!`. This approach is limited,
//! though. It relies on the [default registry] which can't be configured,
//! it requires having a global state, and it's not suitable for libraries.
//!
//! All in all, one usually ends up with something like this:
//!
//! ```
//! struct Metrics {
//!     inflight: prometheus::IntGauge,
//!     requests_duration_seconds: prometheus::Histogram,
//! };
//!
//! impl Metrics {
//!     fn new(registry: &prometheus::Registry) -> prometheus::Result<Self> {
//!         let opts = prometheus::Opts::new(
//!             "inflight",
//!             "Number of requests that are currently inflight."
//!         );
//!         let inflight = prometheus::IntGauge::with_opts(opts)?;
//!
//!         let opts = prometheus::HistogramOpts::new(
//!             "requests_duration_seconds",
//!             "Processing time of each request in seconds."
//!         );
//!         let requests_duration_seconds = prometheus::Histogram::with_opts(opts)?;
//!
//!         Ok(Self {
//!             inflight,
//!             requests_duration_seconds,
//!         })
//!     }
//! }
//! ```
//!
//! This crate provides a derive macro that can automatically
//! generate the `new` function for the above struct.
//!
//! # Quickstart
//!
//! Define a struct that contains all metrics for a component
//! and derive the [`MetricStorage`] trait:
//!
//! ```
//! # use prometheus_metric_storage::MetricStorage;
//! #[derive(MetricStorage)]
//! struct Metrics {
//!     /// Number of requests that are currently inflight.
//!     inflight: prometheus::IntGauge,
//!
//!     /// Processing time of each request in seconds.
//!     requests_duration_seconds: prometheus::Histogram,
//! }
//! ```
//!
//! Now you can instantiate this struct and register all metrics without
//! having to write lots of boilerplate code:
//!
//! ```
//! # use prometheus_metric_storage::MetricStorage;
//! # #[derive(MetricStorage)]
//! # struct Metrics {
//! #     /// Number of requests that are currently inflight.
//! #     inflight: prometheus::IntGauge,
//! #     /// Processing time of each request in seconds.
//! #     requests_duration_seconds: prometheus::Histogram,
//! # }
//! let registry = prometheus::Registry::default();
//! let metrics = Metrics::new(&registry).unwrap();
//! metrics.inflight.inc();
//! metrics.requests_duration_seconds.observe(0.25);
//! ```
//!
//! Field names become metric names, and first line of each of the field's
//! documentation becomes metric's help message. Additional configuration
//! can be done via the [`#[metric(...)]`](#configuring-metrics) attribute.
//!
//! So, the code above will report the following metrics:
//!
//! ```text
//! # HELP inflight Number of requests that are currently inflight.
//! # TYPE inflight gauge
//! inflight 1
//! # HELP requests_duration_seconds Processing time of each request in seconds.
//! # TYPE requests_duration_seconds histogram
//! requests_duration_seconds_bucket{le="0.005"} 0
//! ...
//! requests_duration_seconds_sum 0.25
//! requests_duration_seconds_count 1
//! ```
//!
//! # Generated code API
//!
//! The derive macro will automatically generate implementation
//! for the [`MetricStorage`] trait. On top of it, it will generate
//! three more methods:
//!
//! - <code>fn new(registry: &[Registry], ...) -> [`Result`]\<Self\></code>:
//!
//!   Creates a new instance of a metric storage and registers all of its metrics
//!   in the given registry via [`MetricStorage::register`].
//!
//!   This method accepts a reference to a registry, and const labels,
//!   if storage defines any (see section on [configuring metrics](#configuring-metrics),
//!   and also the [`const_labels`] field of the [`prometheus::Opts`] struct).
//!
//!   Const label parameters should implement <code>[Into]\<[String]\></code>,
//!   they are listed in the same order as they appear
//!   in the `#[metric(labels(...))]` attribute.
//!
//! - <code>fn new_unregistered(...) -> [Result]\<Self\></code>:
//!
//!   Same as `new`, but doesn't add metrics to any registry. You can use
//!   [`MetricStorage::register`] to register metrics later.
//!
//! - <code>fn instance(registry: &[StorageRegistry], ...) -> [Result]\<&Self\></code>:
//!
//!   Looks up storage with the given const label values in a [`StorageRegistry`],
//!   creates one if it's not found.
//!
//!   See [`StorageRegistry::get_or_create_storage`] for more info.
//!
//! # Configuring metrics
//!
//! Additional configuration can be done via the `#[metric(...)]` attribute.
//!
//! On the struct level the available keys are the following:
//!
//! - **subsystem** — a string that will be prepended to each metrics' name.
//!
//!   For example, consider the following storage:
//!
//!   ```
//!   # use prometheus_metric_storage_derive::MetricStorage;
//!   #[derive(MetricStorage)]
//!   #[metric(subsystem = "transport")]
//!   struct Metrics {
//!       /// Processing time of each request in seconds.
//!       requests_duration_seconds: prometheus::Histogram,
//!   }
//!   ```
//!
//!   Here, the metric will be named `transport_requests_duration_seconds`.
//!
//!   See the [`subsystem`] field of the [`prometheus::Opts`] struct for more
//!   info on components that constitute a metric name.
//!
//! - **labels** — a list of const labels that will be added to each metric.
//!
//!   These labels should be provided during the storage initialization.
//!   They allow registering multiple metrics with the same name and different
//!   const label values. Or, in case of this crate, creating and registering
//!   multiple instances of the same metric storage.
//!
//!   Note, however, that trying to register the same storage with the same
//!   const label values twice will still lead to an "already registered" error.
//!   To bypass this, use [metric storage registry](#metric-storage-registry).
//!
//!   Example:
//!
//!   ```
//!   # use prometheus_metric_storage_derive::MetricStorage;
//!   #[derive(MetricStorage)]
//!   #[metric(labels("url"))]
//!   struct Metrics {
//!   # /*
//!      ...
//!   # */
//!   }
//!
//!   # let registry = prometheus::Registry::default();
//!   let google_metrics = Metrics::new(&registry, "https://google.com/").unwrap();
//!
//!   // This will not return an error because we're using a different label value.
//!   let duckduckgo_metrics = Metrics::new(&registry, "https://duckduckgo.com/").unwrap();
//!
//!   // This will return an error because metric storage with the same label value
//!   // is already registered:
//!   // let google_metrics_2 = Metrics::new(&registry, "https://google.com/").unwrap();
//!   ```
//!
//!   See the [`const_labels`] field of the [`prometheus::Opts`] struct for more
//!   info on different label settings.
//!
//! On the field level, the following options are available:
//!
//! - **name** — a string that overrides metric name derived from the field name.
//!
//!   This is useful for tuple structs:
//!
//!   ```
//!   # use prometheus_metric_storage_derive::MetricStorage;
//!   #[derive(MetricStorage)]
//!   struct Metrics (
//!       #[metric(name = "requests", help = "Number of successful requests.")]
//!       prometheus::IntCounter
//!   );
//!   ```
//!
//!   Note that this setting does not override `subsystem` configuration.
//!   That is, `subsystem` will still be prepended to metric's name.
//!
//! - **help** — a string that overrides help message derived
//!   from documentation.
//!
//! - **labels** — a list of strings that will be used as labels for
//!   multidimensional (`Vec`) metrics. Order of labels will be preserved,
//!   so you can rely on it in functions such as [`MetricVec::with_label_values`].
//!
//!   Example:
//!
//!   ```
//!   # use prometheus_metric_storage_derive::MetricStorage;
//!   # #[derive(MetricStorage)]
//!   # struct Metrics {
//!   # /// -
//!   #[metric(labels("url", "status"))]
//!   requests_finished: prometheus::IntCounterVec,
//!   # }
//!   ```
//!
//! - **buckets** — a list of floating point numbers used as histogram
//!   bucket bounds. Numbers should be listed in ascending order.
//!
//!   Example:
//!
//!   ```
//!   # use prometheus_metric_storage_derive::MetricStorage;
//!   # #[derive(MetricStorage)]
//!   # struct Metrics {
//!   # /// -
//!   #[metric(buckets(0.1, 0.2, 0.5, 1, 2, 4, 8))]
//!   requests_duration_seconds: prometheus::Histogram,
//!   # }
//!   ```
//!
//! # Supporting custom collectors
//!
//! If your project uses custom [collectors], metric storage will not be able
//! to instantiate them by default. You'll have to implement [`MetricInit`]
//! and possibly [`HistMetricInit`] for each of the collector you wish to use.
//!
//! # Metric storage registry
//!
//! When registering a metric storage, there's a requirement
//! that a single metric should not be registered twice within
//! the same registry. In practice, this means that, once a storage has been
//! created and registered, it should not be created again:
//!
//! ```should_panic
//! # use prometheus_metric_storage::MetricStorage;
//! # #[derive(MetricStorage)]
//! # struct Metrics {
//! #     /// -
//! #     pieces_of_stuff_processed: prometheus::IntCounter,
//! # }
//! fn do_stuff() {
//! # /*
//!     ...
//! # */
//!
//!     let metrics = Metrics::new(prometheus::default_registry()).unwrap();
//!     metrics.pieces_of_stuff_processed.inc();
//! }
//!
//! # /*
//! ...
//! # */
//!
//! # fn main() {
//! // The first call will work just fine.
//! do_stuff();
//!
//! // The second call will panic, though, because the metrics
//! // were already registered.
//! do_stuff();
//! # }
//! ```
//!
//! There are two approaches to solve this issue.
//!
//! The first one is to create a static variable within the `do_stuff`'s body.
//! As was pointed out in the [section about motivation](#motivation),
//! the code will have to rely on the [default registry], so this is not
//! suitable for libraries.
//!
//! The second one is to have `do_stuff` accept a reference to `Metrics`.
//! This solution complicates component's public API, exposes implementation
//! details of metric collection.
//!
//! To give a better way of dealing with the situation, this crate provides
//! [`StorageRegistry`]: a wrapper around the default registry that keeps track
//! of all created storages, and makes sure that a single storage
//! is only registered once:
//!
//! ```
//! # use prometheus_metric_storage::{StorageRegistry, MetricStorage};
//! # #[derive(MetricStorage)]
//! # struct Metrics {
//! #     /// -
//! #     pieces_of_stuff_processed: prometheus::IntCounter,
//! # }
//! fn do_stuff(registry: &StorageRegistry) {
//! # /*
//!     ...
//! # */
//!
//!     let metrics = Metrics::instance(registry).unwrap();
//!     metrics.pieces_of_stuff_processed.inc();
//! }
//!
//! # /*
//! ...
//! # */
//!
//! # fn main() {
//! let registry = StorageRegistry::default();
//!
//! // The first call will work just fine.
//! do_stuff(&registry);
//!
//! // The second call will also work, because `StorageRegistry`
//! // makes sure not to register storages twice.
//! do_stuff(&registry);
//! # }
//! ```
//!
//! [static metrics]: prometheus#static-metrics
//! [default registry]: prometheus::default_registry
//! [collectors]: prometheus::core::Collector
//! [`subsystem`]: prometheus::Opts#structfield.subsystem
//! [`const_labels`]: prometheus::Opts#structfield.const_labels
//! [`MetricVec::with_label_values`]: prometheus::core::MetricVec::with_label_values

#![deny(missing_docs)]

#[cfg(doctest)]
mod test_readme {
    #[doc = include_str!("../../README.md")]
    mod test_readme_impl {}
}

use prometheus::core::Collector;
use prometheus::proto::MetricFamily;
use std::any::{Any, TypeId};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::pin::Pin;
use std::sync::Mutex;

#[doc(hidden)]
pub use prometheus::{Error, Opts, Registry, Result};

/// Generates implementation for [`MetricStorage`] and three additional
/// methods: `new`, `new_unregistered`, `instance`.
///
/// See the [crate-level] documentation for more info.
///
/// [crate-level]: crate#generated-code-api
pub use prometheus_metric_storage_derive::MetricStorage;

/// Identifier of a single storage in [`StorageRegistry`].
///
/// Storage ID consists of a type ID and static label values
/// concatenated into a single string with zero bytes as a delimiter.
type StorageId = (TypeId, String);

/// Wrapper for prometheus' [`Registry`] that keeps track of registered
/// storages, and helps to avoid "already registered" errors without
/// having to use lazy statics.
///
/// See the [crate-level] documentation for more info.
///
/// # Limitations
///
/// Calls to [`get_storage`] and [`get_or_create_storage`] involve
/// concatenating values of all const labels into a single string,
/// and looking up this string in a hashtable. Make sure that these
/// calls are out of your hot path.
///
/// [crate-level]: crate#metric-storage-registry
/// [`get_storage`]: StorageRegistry::get_storage
/// [`get_or_create_storage`]: StorageRegistry::get_or_create_storage
pub struct StorageRegistry {
    /// The underlying metrics registry.
    registry: Registry,

    /// Saved registered storages.
    ///
    /// # Safety
    ///
    /// Storages in this hashmap must not be removed or replaced.
    /// They must only be dropped when this registry is dropped.
    storages: Mutex<HashMap<StorageId, Pin<Box<dyn Any + Send + Sync>>>>,
}

impl StorageRegistry {
    /// Create a new storage registry.
    pub fn new(registry: prometheus::Registry) -> Self {
        Self {
            registry,
            storages: Default::default(),
        }
    }

    /// Return a reference to the underlying [`Registry`].
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Convert this wrapper into the underlying [`Registry`].
    ///
    /// All information about registered storages is lost.
    pub fn into_registry(self) -> Registry {
        self.registry
    }

    /// Register a single metric in the underlying registry.
    ///
    /// Return an error if the given metric was already registered
    /// when this function is called.
    ///
    /// See [`Registry::unregister`] for more info.
    pub fn register(&self, c: Box<dyn Collector>) -> Result<()> {
        self.registry.register(c)
    }

    /// Unregister a single metric from the underlying registry.
    ///
    /// Returns an error if the given metric was not registered
    /// when this function is called.
    ///
    /// See [`Registry::unregister`] for more info.
    pub fn unregister(&self, c: Box<dyn Collector>) -> Result<()> {
        self.registry.unregister(c)
    }

    /// Gather all metrics from the underlying registry.
    ///
    /// See [`Registry::gather`] for more info.
    pub fn gather(&self) -> Vec<MetricFamily> {
        self.registry.gather()
    }

    /// Find a storage of the given type with tha given labels.
    ///
    /// Returns an error if the given metric storage was not registered
    /// with the given labels, or if the given labels are invalid.
    pub fn get_storage<T: MetricStorage + Send + Sync + 'static>(
        &self,
        const_labels: HashMap<String, String>,
    ) -> Result<&T> {
        let metric_id = Self::make_id::<T>(&const_labels)?;

        let mut storages = self.storages.lock().unwrap();

        let storage = match storages.entry(metric_id) {
            Entry::Occupied(entry) => entry.into_mut().downcast_ref::<T>().unwrap(),
            Entry::Vacant(_) => {
                return Err(Error::Msg(format!(
                    "metric storage {} not found",
                    std::any::type_name::<T>()
                )))
            }
        };

        // Safety:
        //
        // See `get_or_create_storage` for details.
        unsafe { Ok(&*(storage as *const T)) }
    }

    /// Return a storage of the given type with tha given labels. If such
    /// storage does not exist in this registry, create it and register
    /// its metrics.
    ///
    /// Returns an error if the given labels are invalid or if storage creation
    /// has failed.
    pub fn get_or_create_storage<T: MetricStorage + Send + Sync + 'static>(
        &self,
        const_labels: HashMap<String, String>,
    ) -> Result<&T> {
        let metric_id = Self::make_id::<T>(&const_labels)?;

        let mut storages = self.storages.lock().unwrap();

        let storage = match storages.entry(metric_id) {
            Entry::Occupied(entry) => entry.into_mut().downcast_ref::<T>().unwrap(),
            Entry::Vacant(entry) => {
                let storage = T::from_const_labels(&self.registry, const_labels)?;
                entry.insert(Box::pin(storage)).downcast_ref::<T>().unwrap()
            }
        };

        // Safety:
        //
        // We never remove storages from this registry, thus they will live
        // for as long as this registry lives. We've also made storages
        // `Pin`, so we never move them. This means that a reference
        // to a storage will stay valid for as long as this registry lives.
        //
        // There are no issues with drop check because this registry
        // does not implement custom drop, and the storage is `'static`.
        //
        // It is also ok to unlock mutex guard while holding reference
        // to a storage because the storage is `Send + Sync`.
        //
        // Note that we're not returning a `'static` reference, but rather
        // a reference with the lifetime of `&self`.
        unsafe { Ok(&*(storage as *const T)) }
    }

    fn make_id<T: MetricStorage + Send + Sync + 'static>(
        const_labels: &HashMap<String, String>,
    ) -> Result<StorageId> {
        let labels_spec = T::const_labels();

        if labels_spec.len() != const_labels.len() {
            return Err(Error::Msg(format!(
                "invalid number of const labels: expected {}, got {}",
                labels_spec.len(),
                const_labels.len()
            )));
        }

        let mut values = String::new();

        for &label in T::const_labels() {
            if let Some(value) = const_labels.get(label) {
                values.push_str(value);
                values.push('\0');
            } else {
                return Err(Error::Msg(format!("label {:?} is missing", label)));
            }
        }

        Ok((TypeId::of::<T>(), values))
    }
}

impl Default for StorageRegistry {
    fn default() -> Self {
        Self::new(Registry::new())
    }
}

impl Debug for StorageRegistry {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("StorageRegistry")
    }
}

/// Get the default storage registry that uses [`prometheus::default_registry`].
pub fn default_storage_registry() -> &'static StorageRegistry {
    lazy_static::lazy_static! {
        static ref REGISTRY: StorageRegistry =
            StorageRegistry::new(prometheus::default_registry().clone());
    }

    &REGISTRY as &StorageRegistry
}

/// Common interface for metric storages.
///
/// This trait should be derived with the `#[derive(MetricStorage)]` macro.
pub trait MetricStorage: Sized {
    /// Get array of const labels used in this storage.
    ///
    /// Labels are listed in the same order as they appear
    /// in the `#[metric(labels(...))]` attribute.
    ///
    /// See [crate-level] documentation for more info.
    ///
    /// [crate-level]: crate#configuring-metrics
    fn const_labels() -> &'static [&'static str];

    /// Create a new instance of this storage and register all of its metrics
    /// in the given registry.
    ///
    /// For any given metric storage, this function should not be called twice
    /// with the same values for const labels. Otherwise, the registry will
    /// complain about a metric being registered twice.
    ///
    /// If the given const labels do not match the ones declared
    /// in the `#[metric(labels(...))]` attribute of the struct
    /// that's being created, this function will return an error.
    fn from_const_labels(
        registry: &Registry,
        const_labels: HashMap<String, String>,
    ) -> Result<Self> {
        let storage = Self::from_const_labels_unregistered(const_labels)?;
        storage.register(registry)?;
        Ok(storage)
    }

    /// Create a new instance of this storage and initialize all of its metrics.
    ///
    /// This function does not register the created metrics in any storage.
    ///
    /// If the given const labels do not match the ones declared
    /// in the `metric(labels(...))` attribute of the struct
    /// that's being created, this function will return an error.
    fn from_const_labels_unregistered(const_labels: HashMap<String, String>) -> Result<Self>;

    /// Register all metrics from this storage in the given registry.
    fn register(&self, registry: &Registry) -> Result<()>;
}

/// This trait is used to initialize metrics.
///
/// Generated constructor will pass all its options to this trait's
/// [`init`] function in order to initialize a field. If you're using
/// custom metric collectors, you'll need to implement
/// this trait for them.
///
/// [`init`]: MetricInit::init
pub trait MetricInit: Sized {
    /// Initialize a new instance of the metric using the given options.
    fn init(opts: prometheus::Opts) -> Result<Self>;
}

/// This trait is used to initialize metrics that accept buckets.
///
/// This trait is similar to [`MetricInit`], but accepts histogram-specific
/// options.
///
/// Note that histogram metrics should still be initializeable
/// with [`MetricInit`]. This trait is only used when histogram-specific
/// options appear in the metric config.
pub trait HistMetricInit: Sized {
    /// Initialize a new instance of the metric using the given options.
    fn init(opts: prometheus::Opts, buckets: Vec<f64>) -> Result<Self>;
}

// Impls

impl<T: prometheus::core::Atomic> MetricInit for prometheus::core::GenericGauge<T> {
    fn init(opts: Opts) -> Result<Self> {
        Self::with_opts(opts)
    }
}

impl<T: prometheus::core::Atomic> MetricInit for prometheus::core::GenericCounter<T> {
    fn init(opts: Opts) -> Result<Self> {
        Self::with_opts(opts)
    }
}

impl MetricInit for prometheus::Histogram {
    fn init(opts: Opts) -> Result<Self> {
        Self::with_opts(opts.into())
    }
}

impl<T: prometheus::core::Atomic> MetricInit for prometheus::core::GenericGaugeVec<T> {
    fn init(mut opts: Opts) -> Result<Self> {
        let labels = std::mem::take(&mut opts.variable_labels);
        let labels_view: Vec<_> = labels.iter().map(AsRef::as_ref).collect();
        Self::new(opts, &labels_view)
    }
}

impl<T: prometheus::core::Atomic> MetricInit for prometheus::core::GenericCounterVec<T> {
    fn init(mut opts: Opts) -> Result<Self> {
        let labels = std::mem::take(&mut opts.variable_labels);
        let labels_view: Vec<_> = labels.iter().map(AsRef::as_ref).collect();
        Self::new(opts, &labels_view)
    }
}

impl MetricInit for prometheus::HistogramVec {
    fn init(mut opts: Opts) -> Result<Self> {
        let labels = std::mem::take(&mut opts.variable_labels);
        let labels_view: Vec<_> = labels.iter().map(AsRef::as_ref).collect();
        Self::new(opts.into(), &labels_view)
    }
}

impl HistMetricInit for prometheus::Histogram {
    fn init(opts: Opts, buckets: Vec<f64>) -> Result<Self> {
        let opts: prometheus::HistogramOpts = opts.into();
        Self::with_opts(opts.buckets(buckets))
    }
}

impl HistMetricInit for prometheus::HistogramVec {
    fn init(mut opts: Opts, buckets: Vec<f64>) -> Result<Self> {
        let labels = std::mem::take(&mut opts.variable_labels);
        let labels_view: Vec<_> = labels.iter().map(AsRef::as_ref).collect();
        let opts: prometheus::HistogramOpts = opts.into();
        Self::new(opts.buckets(buckets), &labels_view)
    }
}
