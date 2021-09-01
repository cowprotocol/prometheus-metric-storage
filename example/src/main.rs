use prometheus_metric_storage::MetricStorage;

#[derive(MetricStorage)]
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
    let registry = prometheus::default_registry();

    let metrics: Metrics = Metrics::new(registry, /* endpoint = */ "infura_mainnet".into()).unwrap();
    metrics.inflight.inc();
    metrics.requests_finished.with_label_values(&["200"]).inc();
    metrics.requests_duration_seconds.observe(0.015);

    dbg!(registry.gather());
}
