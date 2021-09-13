# Prometheus metric storage

[![tests](https://github.com/taminomara/prometheus-metric-storage/actions/workflows/tests.yaml/badge.svg?branch=main)](https://github.com/taminomara/prometheus-metric-storage/actions/workflows/tests.yaml)

---

- [Crates.io](https://crates.io/crates/prometheus-metric-storage)
- [Documentation](https://docs.rs/prometheus-metric-storage/)

---

When instrumenting code with prometheus metrics, one is required
to write quite a bit of boilerplate code.

This crate will generate most of said boilerplate for you:

```rust
#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "transport", labels("endpoint"))]
struct Metrics {
    /// Number of requests that are currently inflight.
    inflight: prometheus::IntGauge,

    /// Number of finished requests by response code.
    #[metric(labels("status"))]
    requests_finished: prometheus::IntCounterVec,

    /// Number of finished requests by total processing duration.
    #[metric(buckets(0.1, 0.2, 0.5, 1, 2, 4, 8))]
    requests_duration_seconds: prometheus::Histogram,
}

fn main() {
    let metrics = Metrics::new(
        prometheus::default_registry(),
        /* endpoint = */ "0.0.0.0:8080"
    ).unwrap();

    metrics.inflight.inc();
    metrics.requests_finished.with_label_values(&["200"]).inc();
    metrics.requests_duration_seconds.observe(0.015);
}
```
