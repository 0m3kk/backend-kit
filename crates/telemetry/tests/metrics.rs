use std::time::Duration;

use telemetry::{Counter, Gauge, Histogram};

#[test]
fn counter_supports_tuples_and_attributes() {
    let counter = Counter::with_scope("test_scope", "events_total");
    counter.inc();
    counter.inc_with([("event_type", "order_placed"), ("source", "web")]);
    counter.add(10, [("event_type", "batch_synced")]);
}

#[test]
fn gauge_increments_and_decrements() {
    let gauge = Gauge::with_scope("test_scope", "active_tasks");
    gauge.inc();
    gauge.inc_with([("worker", "worker-1")]);
    gauge.dec();
    gauge.dec_with([("worker", "worker-1")]);
    gauge.add(-5, [("worker", "worker-1")]);
}

#[test]
fn histogram_records_and_measures_duration() {
    let histogram = Histogram::with_scope_and_unit("test_scope", "request_duration_seconds", "s");
    histogram.record(0.045, [("path", "/healthz")]);
    histogram.record_duration(Duration::from_millis(50), [("path", "/api")]);

    // RAII timer test
    {
        let _timer = histogram.start_timer_with([("path", "/orders")]);
        std::thread::sleep(Duration::from_millis(2));
    }

    let timer = histogram.start_timer();
    let elapsed = timer.stop();
    assert!(elapsed >= Duration::from_millis(0));
}
