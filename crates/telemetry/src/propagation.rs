//! W3C TraceContext propagation helpers.

use std::collections::HashMap;

use opentelemetry::propagation::{Extractor, Injector};
use opentelemetry_sdk::propagation::TraceContextPropagator;

/// Configures the global W3C TraceContext propagator.
pub fn init_propagator() {
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
}

/// Helper implementing [`Extractor`] for key-value maps.
pub struct HeaderExtractor<'a>(pub &'a HashMap<String, String>);

impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(|s| s.as_str())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

/// Helper implementing [`Injector`] for key-value maps.
pub struct HeaderInjector<'a>(pub &'a mut HashMap<String, String>);

impl Injector for HeaderInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }
}

/// Extracts an OpenTelemetry [`Context`](opentelemetry::Context) from map headers using W3C TraceContext.
pub fn extract_context(headers: &HashMap<String, String>) -> opentelemetry::Context {
    let extractor = HeaderExtractor(headers);
    opentelemetry::global::get_text_map_propagator(|propagator| propagator.extract(&extractor))
}

/// Injects an OpenTelemetry [`Context`](opentelemetry::Context) into map headers using W3C TraceContext.
pub fn inject_context(cx: &opentelemetry::Context, headers: &mut HashMap<String, String>) {
    let mut injector = HeaderInjector(headers);
    opentelemetry::global::get_text_map_propagator(|propagator| {
        propagator.inject_context(cx, &mut injector);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_and_inject_context_roundtrip() {
        init_propagator();

        let mut headers = HashMap::new();
        headers.insert(
            "traceparent".to_string(),
            "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string(),
        );

        let cx = extract_context(&headers);

        let mut out_headers = HashMap::new();
        inject_context(&cx, &mut out_headers);

        assert_eq!(
            out_headers.get("traceparent"),
            Some(&"00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01".to_string())
        );
    }
}
