use prometheus_metric_storage::{MetricStorage, StorageRegistry};

#[derive(Clone, MetricStorage)]
#[metric(subsystem = "transport", labels("endpoint"))]
struct Metrics {
    /// Number of requests that are currently inflight.
    inflight: prometheus::IntGauge,

    /// Number of finished requests by response code.
    #[metric(labels("status"))]
    requests_finished: prometheus::IntCounterVec,

    /// Number of finished requests by total processing duration.
    requests_duration_seconds: prometheus::Histogram,
}

fn main() {
    let registry = StorageRegistry::default();

    let metrics = Metrics::instance(&registry, "infura_mainnet".into()).unwrap();
    metrics.inflight.inc();
    metrics.requests_finished.with_label_values(&["200"]).inc();
    metrics.requests_duration_seconds.observe(0.015);

    let metrics = Metrics::instance(&registry, "infura_rinkeby".into()).unwrap();
    metrics.inflight.inc();
    metrics.requests_finished.with_label_values(&["200"]).inc();
    metrics.requests_duration_seconds.observe(0.025);

    let metrics = Metrics::instance(&registry, "infura_mainnet".into()).unwrap();
    metrics.inflight.inc();

    dbg!(registry.gather());
}
