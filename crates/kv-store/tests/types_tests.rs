use kv_store::{BatchOp, Key, KvEntry, ScanOptions, SetOptions, Value};
use std::time::{Duration, SystemTime};

#[test]
fn test_key_value_conversions() {
    let key = Key::from("user:100");
    assert_eq!(key.as_str(), Some("user:100"));
    assert_eq!(key.to_string(), "user:100");

    let val = Value::from("Alice");
    assert_eq!(val.as_str(), Some("Alice"));
}

#[test]
fn test_entry_expiration() {
    let entry =
        KvEntry::new("k1", "v1").with_expires_at(Some(SystemTime::now() - Duration::from_secs(10)));
    assert!(entry.is_expired());

    let entry_future = KvEntry::new("k2", "v2")
        .with_expires_at(Some(SystemTime::now() + Duration::from_secs(3600)));
    assert!(!entry_future.is_expired());
}

#[test]
fn test_options_builders() {
    let opts = SetOptions::new()
        .with_ttl(Duration::from_secs(60))
        .if_not_exists();
    assert_eq!(opts.ttl, Some(Duration::from_secs(60)));
    assert!(opts.if_not_exists);
    assert!(!opts.if_exists);

    let scan = ScanOptions::new()
        .with_prefix("user:")
        .with_limit(10)
        .reverse();
    assert_eq!(scan.prefix, Some(Key::from("user:")));
    assert_eq!(scan.limit, Some(10));
    assert!(scan.reverse);

    let _op = BatchOp::Put {
        key: Key::from("k"),
        value: Value::from("v"),
        options: opts,
    };
}
