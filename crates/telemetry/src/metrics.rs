//! Ergonomic metrics wrappers and RAII timer utilities.

use std::time::Instant;

use opentelemetry::global;
use opentelemetry::metrics::{
    Counter as OtelCounter, Gauge as OtelGauge, Histogram as OtelHistogram,
    UpDownCounter as OtelUpDownCounter,
};
use opentelemetry::{Key, KeyValue, Value};

/// Helper trait to convert various attribute types into OpenTelemetry [`KeyValue`].
pub trait IntoKeyValue {
    fn into_key_value(self) -> KeyValue;
}

impl IntoKeyValue for KeyValue {
    fn into_key_value(self) -> KeyValue {
        self
    }
}

impl<K, V> IntoKeyValue for (K, V)
where
    K: Into<Key>,
    V: Into<Value>,
{
    fn into_key_value(self) -> KeyValue {
        KeyValue::new(self.0, self.1)
    }
}

/// Converts an iterator of attribute pairs into a vector of [`KeyValue`].
pub fn to_key_values<I, T>(attributes: I) -> Vec<KeyValue>
where
    I: IntoIterator<Item = T>,
    T: IntoKeyValue,
{
    attributes
        .into_iter()
        .map(IntoKeyValue::into_key_value)
        .collect()
}

/// Ergonomic wrapper around an OpenTelemetry monotonically increasing [`Counter`](OtelCounter).
#[derive(Clone)]
pub struct Counter {
    inner: OtelCounter<u64>,
}

impl Counter {
    /// Creates a new counter with default global meter scope.
    pub fn new(name: &'static str) -> Self {
        let meter = global::meter("telemetry");
        Self {
            inner: meter.u64_counter(name).build(),
        }
    }

    /// Creates a new counter with a custom scope name.
    pub fn with_scope(scope: &'static str, name: &'static str) -> Self {
        let meter = global::meter(scope);
        Self {
            inner: meter.u64_counter(name).build(),
        }
    }

    /// Increments the counter by 1 without attributes.
    pub fn inc(&self) {
        self.inner.add(1, &[]);
    }

    /// Increments the counter by 1 with attributes.
    pub fn inc_with<I, T>(&self, attributes: I)
    where
        I: IntoIterator<Item = T>,
        T: IntoKeyValue,
    {
        let kvs = to_key_values(attributes);
        self.inner.add(1, &kvs);
    }

    /// Adds a specific count without attributes.
    pub fn add_count(&self, count: u64) {
        self.inner.add(count, &[]);
    }

    /// Adds a specific count with attributes.
    pub fn add<I, T>(&self, count: u64, attributes: I)
    where
        I: IntoIterator<Item = T>,
        T: IntoKeyValue,
    {
        let kvs = to_key_values(attributes);
        self.inner.add(count, &kvs);
    }
}

/// Ergonomic wrapper around an OpenTelemetry bidirectional [`UpDownCounter`](OtelUpDownCounter).
#[derive(Clone)]
pub struct Gauge {
    inner: OtelUpDownCounter<i64>,
}

impl Gauge {
    /// Creates a new gauge with default global meter scope.
    pub fn new(name: &'static str) -> Self {
        let meter = global::meter("telemetry");
        Self {
            inner: meter.i64_up_down_counter(name).build(),
        }
    }

    /// Creates a new gauge with a custom scope name.
    pub fn with_scope(scope: &'static str, name: &'static str) -> Self {
        let meter = global::meter(scope);
        Self {
            inner: meter.i64_up_down_counter(name).build(),
        }
    }

    /// Increments the gauge by 1 without attributes.
    pub fn inc(&self) {
        self.inner.add(1, &[]);
    }

    /// Increments the gauge by 1 with attributes.
    pub fn inc_with<I, T>(&self, attributes: I)
    where
        I: IntoIterator<Item = T>,
        T: IntoKeyValue,
    {
        let kvs = to_key_values(attributes);
        self.inner.add(1, &kvs);
    }

    /// Decrements the gauge by 1 without attributes.
    pub fn dec(&self) {
        self.inner.add(-1, &[]);
    }

    /// Decrements the gauge by 1 with attributes.
    pub fn dec_with<I, T>(&self, attributes: I)
    where
        I: IntoIterator<Item = T>,
        T: IntoKeyValue,
    {
        let kvs = to_key_values(attributes);
        self.inner.add(-1, &kvs);
    }

    /// Adjusts the gauge by a delta value with attributes.
    pub fn add<I, T>(&self, value: i64, attributes: I)
    where
        I: IntoIterator<Item = T>,
        T: IntoKeyValue,
    {
        let kvs = to_key_values(attributes);
        self.inner.add(value, &kvs);
    }
}

/// Ergonomic wrapper around an OpenTelemetry synchronous [`Gauge`](OtelGauge) that records absolute values.
#[derive(Clone)]
pub struct ValueGauge {
    inner: OtelGauge<u64>,
}

impl ValueGauge {
    /// Creates a new value gauge with default global meter scope.
    pub fn new(name: &'static str) -> Self {
        let meter = global::meter("telemetry");
        Self {
            inner: meter.u64_gauge(name).build(),
        }
    }

    /// Creates a new value gauge with a custom scope name.
    pub fn with_scope(scope: &'static str, name: &'static str) -> Self {
        let meter = global::meter(scope);
        Self {
            inner: meter.u64_gauge(name).build(),
        }
    }

    /// Records an absolute value without attributes.
    pub fn record(&self, value: u64) {
        self.inner.record(value, &[]);
    }

    /// Records an absolute value with attributes.
    pub fn record_with<I, T>(&self, value: u64, attributes: I)
    where
        I: IntoIterator<Item = T>,
        T: IntoKeyValue,
    {
        let kvs = to_key_values(attributes);
        self.inner.record(value, &kvs);
    }
}

/// Ergonomic wrapper around an OpenTelemetry [`Histogram`](OtelHistogram).
#[derive(Clone)]
pub struct Histogram {
    inner: OtelHistogram<f64>,
}

impl Histogram {
    /// Creates a new histogram with default global meter scope.
    pub fn new(name: &'static str) -> Self {
        let meter = global::meter("telemetry");
        Self {
            inner: meter.f64_histogram(name).with_unit("s").build(),
        }
    }

    /// Creates a new histogram with a custom scope name and unit.
    pub fn with_scope_and_unit(
        scope: &'static str,
        name: &'static str,
        unit: &'static str,
    ) -> Self {
        let meter = global::meter(scope);
        Self {
            inner: meter.f64_histogram(name).with_unit(unit).build(),
        }
    }

    /// Records a raw value without attributes.
    pub fn record_value(&self, value: f64) {
        self.inner.record(value, &[]);
    }

    /// Records a raw value with attributes.
    pub fn record<I, T>(&self, value: f64, attributes: I)
    where
        I: IntoIterator<Item = T>,
        T: IntoKeyValue,
    {
        let kvs = to_key_values(attributes);
        self.inner.record(value, &kvs);
    }

    /// Records a duration in seconds with attributes.
    pub fn record_duration<I, T>(&self, duration: std::time::Duration, attributes: I)
    where
        I: IntoIterator<Item = T>,
        T: IntoKeyValue,
    {
        self.record(duration.as_secs_f64(), attributes);
    }

    /// Starts a timer that automatically records elapsed seconds when dropped.
    pub fn start_timer(&self) -> HistogramTimer {
        HistogramTimer {
            histogram: self.clone(),
            start: Instant::now(),
            attributes: Vec::new(),
        }
    }

    /// Starts a timer with pre-configured attributes that automatically records elapsed seconds when dropped.
    pub fn start_timer_with<I, T>(&self, attributes: I) -> HistogramTimer
    where
        I: IntoIterator<Item = T>,
        T: IntoKeyValue,
    {
        HistogramTimer {
            histogram: self.clone(),
            start: Instant::now(),
            attributes: to_key_values(attributes),
        }
    }
}

/// RAII timer guard that records elapsed time to a [`Histogram`] on drop.
pub struct HistogramTimer {
    histogram: Histogram,
    start: Instant,
    attributes: Vec<KeyValue>,
}

impl HistogramTimer {
    /// Manually stops the timer and records the elapsed seconds, returning the duration.
    pub fn stop(self) -> std::time::Duration {
        let elapsed = self.start.elapsed();
        self.histogram
            .inner
            .record(elapsed.as_secs_f64(), &self.attributes);
        // Avoid double recording in drop
        std::mem::forget(self);
        elapsed
    }
}

impl Drop for HistogramTimer {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed().as_secs_f64();
        self.histogram.inner.record(elapsed, &self.attributes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_operations() {
        let counter = Counter::new("test_counter");
        counter.inc();
        counter.inc_with([("status", "success")]);
        counter.add(5, [("status", "pending"), ("env", "test")]);
    }

    #[test]
    fn gauge_operations() {
        let gauge = Gauge::new("test_gauge");
        gauge.inc();
        gauge.inc_with([("pool", "primary")]);
        gauge.dec();
        gauge.dec_with([("pool", "primary")]);
        gauge.add(10, [("pool", "primary")]);
    }

    #[test]
    fn histogram_and_timer_operations() {
        let histogram = Histogram::new("test_histogram");
        histogram.record(0.123, [("endpoint", "/healthz")]);

        {
            let _timer = histogram.start_timer();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        let timer = histogram.start_timer_with([("route", "/api/v1")]);
        let elapsed = timer.stop();
        assert!(elapsed >= std::time::Duration::from_millis(0));
    }
}
