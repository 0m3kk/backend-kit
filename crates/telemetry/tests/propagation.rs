use std::collections::HashMap;

use telemetry::{extract_context, init_propagator, inject_context};

#[test]
fn extracts_and_injects_valid_w3c_traceparent() {
    init_propagator();

    let mut incoming = HashMap::new();
    incoming.insert(
        "traceparent".to_string(),
        "00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01".to_string(),
    );

    let cx = extract_context(&incoming);

    let mut outgoing = HashMap::new();
    inject_context(&cx, &mut outgoing);

    assert_eq!(
        outgoing.get("traceparent"),
        Some(&"00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01".to_string())
    );
}

#[test]
fn handles_missing_traceparent_gracefully() {
    init_propagator();

    let incoming = HashMap::new();
    let cx = extract_context(&incoming);

    let mut outgoing = HashMap::new();
    inject_context(&cx, &mut outgoing);

    assert!(!outgoing.contains_key("traceparent"));
}
