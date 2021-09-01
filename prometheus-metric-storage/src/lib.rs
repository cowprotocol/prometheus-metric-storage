#[doc(hidden)]
pub use prometheus::{Opts, Registry, Result};

#[doc(hidden)]
pub use prometheus_metric_storage_derive::MetricStorage;

/// This trait is used to initialize metrics.
///
/// Generated constructor will pass all its options to this trait's
/// [init] function in order to initialize a field.
pub trait MetricInit: Sized {
    fn init(opts: prometheus::Opts) -> Result<Self>;
}

pub trait HistMetricInit: Sized {
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
