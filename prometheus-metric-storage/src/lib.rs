//! Derive macro to instantiate prometheus metrics with ease.
// TODO: docs

#![deny(missing_docs)]

use prometheus::core::Collector;
use prometheus::proto::MetricFamily;
use std::any::{Any, TypeId};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::pin::Pin;
use std::sync::Mutex;

/// Re-export parts of prometheus interface for use in generated code.
#[doc(hidden)]
pub use prometheus::{Error, Opts, Registry, Result};

#[doc(hidden)]
pub use prometheus_metric_storage_derive::MetricStorage;

/// Identifier of a single storage in [`StorageRegistry`].
///
/// Storage ID consists of a type ID and static label values
/// concatenated into a single string with zero bytes as a delimiter.
type StorageId = (TypeId, String);

/// Wrapper for prometheus' [`Registry`] that keeps track of registered
/// storages, and helps to avoid 'already registered' errors without
/// having to use lazy statics.
pub struct StorageRegistry {
    registry: Registry,
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
    /// Return an error if the given metric was not registered
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

    /// Returns a storage of the given type with tha given labels.
    ///
    /// Returns an error if the given metric storage was not registered
    /// with the given labels, or if the given labels are invalid.
    pub fn get_storage<T: MetricStorage + Send + Sync + 'static>(
        &self,
        const_labels: HashMap<String, String>,
    ) -> Result<&T> {
        // Safety:
        //
        // See `get_or_create_storage` for details.
        unsafe {
            let metric_id = Self::make_id::<T>(&const_labels)?;

            let mut storages = self.storages.lock().unwrap();

            let storage = match storages.entry(metric_id) {
                Entry::Occupied(entry) => entry.into_mut().downcast_ref::<T>().unwrap(),
                Entry::Vacant(_) => {
                    return Err(Error::Msg(format!(
                        "metrics storage {} not found",
                        std::any::type_name::<T>()
                    )))
                }
            };

            Ok(&*(storage as *const T))
        }
    }

    /// Returns a storage of the given type with tha given labels. If such
    /// storage does not exist in this registry, creates it and registers
    /// its metrics.
    ///
    /// Returns an error if the given labels are invalid or if storage creation
    /// has failed.
    pub fn get_or_create_storage<T: MetricStorage + Send + Sync + 'static>(
        &self,
        const_labels: HashMap<String, String>,
    ) -> Result<&T> {
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
        unsafe {
            let metric_id = Self::make_id::<T>(&const_labels)?;

            let mut storages = self.storages.lock().unwrap();

            let storage = match storages.entry(metric_id) {
                Entry::Occupied(entry) => entry.into_mut().downcast_ref::<T>().unwrap(),
                Entry::Vacant(entry) => {
                    let storage = T::from_const_labels(&self.registry, const_labels)?;
                    entry.insert(Box::pin(storage)).downcast_ref::<T>().unwrap()
                }
            };

            Ok(&*(storage as *const T))
        }
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

/// This trait should be derived with `#[derive]` statement.
pub trait MetricStorage: Sized {
    /// Get array of const labels used in this storage.
    ///
    /// Labels are listed in the same order as they appear
    /// in derive-macro attribute.
    fn const_labels() -> &'static [&'static str];

    /// Create a new instance of this storage and register all of its metrics
    /// in the given registry.
    ///
    /// For any given metric storage, this function should not be called twice
    /// with the same values for const labels. Otherwise, the registry will
    /// complain about a metric being registered twice.
    ///
    /// If the given const labels do not match the ones declared
    /// in the `metric(labels(...))` attribute of the struct
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
/// with [`MetricInit`]. This trait is used only when histogram-specific
/// options appear in metric config.
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
