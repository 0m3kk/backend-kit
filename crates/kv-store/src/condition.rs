use serde_json::Value;

/// An atomic condition for a [put](crate::KvStore::put).
///
/// Use [`PutCondition::Absent`] for create-if-absent and
/// [`PutCondition::Value`] for compare-and-swap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PutCondition {
    /// The put only succeeds if the key does not exist.
    Absent,
    /// The put only succeeds if the current value equals `value`.
    Value(Value),
}
