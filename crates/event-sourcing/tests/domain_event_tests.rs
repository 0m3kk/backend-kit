#![allow(clippy::expect_used, clippy::unwrap_used)]

use event_sourcing::*;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct OrderCreated {
    order_id: String,
    total_amount: f64,
}

impl DomainEvent for OrderCreated {
    fn event_type() -> EventType {
        EventType::new("OrderCreated")
    }

    fn tags(&self) -> Vec<Tag> {
        vec![Tag::key_value("order", &self.order_id)]
    }
}

#[test]
fn test_domain_event_trait_and_conversion() {
    let domain_evt = OrderCreated {
        order_id: "ORD-123".to_string(),
        total_amount: 199.99,
    };

    let event = domain_evt.to_event("evt-1").unwrap();
    assert_eq!(event.id.as_str(), "evt-1");
    assert_eq!(event.event_type.as_str(), "OrderCreated");
    assert_eq!(event.tags, vec![Tag::new("order:ORD-123")]);

    let restored: OrderCreated = event.to_domain_event().unwrap();
    assert_eq!(restored, domain_evt);

    let seq_evt = SequencedEvent::new(SequencePosition::new(1), 1000, event);
    let seq_restored: OrderCreated = seq_evt.to_domain_event().unwrap();
    assert_eq!(seq_restored, domain_evt);
}
