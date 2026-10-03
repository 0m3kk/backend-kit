use std::collections::{BTreeMap, BTreeSet};

use crate::event::{Event, SequencePosition, Tags};

/// A single item of a [Query].
///
/// An event matches if its type matches one of the types and its tags contain
/// all of the tags. Each constraint is optional: an empty `types` or `tags`
/// list matches anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryItem {
    /// Event types the event must match (at least one). Empty matches any type.
    pub types: Vec<String>,
    /// Tags the event must contain (all of them). Empty matches any tags.
    pub tags: Tags,
}

impl QueryItem {
    /// Create a new query item.
    pub fn new(types: Vec<String>, tags: Tags) -> Self {
        Self { types, tags }
    }

    /// Check whether an event matches this query item.
    pub fn matches(&self, event: &Event) -> bool {
        let type_matches =
            self.types.is_empty() || self.types.iter().any(|t| t == &event.event_type);
        let tags_match = self
            .tags
            .iter()
            .all(|tag| event.tags.iter().any(|t| t == tag));

        type_matches && tags_match
    }

    /// Check whether an event type matches this query item.
    pub fn matches_event_type(&self, event_type: &str) -> bool {
        self.types.is_empty() || self.types.iter().any(|t| t == event_type)
    }

    /// Stable fingerprint of this query item, with types and tags sorted.
    fn fingerprint(&self) -> String {
        if self.types.is_empty() && self.tags.is_empty() {
            return "*".to_string();
        }

        let mut parts = vec![];
        if !self.types.is_empty() {
            let mut types = self.types.clone();
            types.sort();
            parts.push(types.join(","));
        }
        if !self.tags.is_empty() {
            let mut tags = self.tags.clone();
            tags.sort();
            parts.push(format!("@{}", tags.join(",")));
        }
        parts.join(":")
    }
}

/// Constraints for filtering events.
///
/// Either [`Query::all`] (matches all events) or a set of [QueryItem]s
/// combined with an **OR**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Query {
    /// Matches all events.
    All,
    /// Matches events that match at least one of the items.
    Items(Vec<QueryItem>),
}

impl Query {
    /// Create a query that matches all events.
    pub fn all() -> Self {
        Self::All
    }

    /// Create a query from a set of query items.
    ///
    /// An event matches if it matches at least one of the items.
    pub fn from_items(items: impl IntoIterator<Item = QueryItem>) -> Self {
        Self::Items(items.into_iter().collect())
    }

    /// Check whether an event matches this query.
    pub fn matches(&self, event: &Event) -> bool {
        match self {
            Self::All => true,
            Self::Items(items) => items.iter().any(|item| item.matches(event)),
        }
    }

    /// Check whether an event type matches this query.
    pub fn matches_event_type(&self, event_type: &str) -> bool {
        match self {
            Self::All => true,
            Self::Items(items) => items.iter().any(|item| item.matches_event_type(event_type)),
        }
    }

    /// Stable, order-independent fingerprint of this query.
    ///
    /// Equal queries always produce the same fingerprint, regardless of the
    /// order of items, types or tags. Useful as a subscription or cache key.
    pub fn fingerprint(&self) -> String {
        match self {
            Self::All => "all".to_string(),
            Self::Items(items) => {
                let mut items: Vec<_> = items.iter().map(QueryItem::fingerprint).collect();
                items.sort();
                items.join("|")
            }
        }
    }

    /// Combine queries into one covering all of them (OR).
    ///
    /// The result is optimized: redundant items are merged or dropped.
    pub fn combine(queries: impl IntoIterator<Item = Query>) -> Self {
        let mut items = Vec::new();
        for query in queries {
            match query {
                Self::All => return Self::All,
                Self::Items(query_items) => items.extend(query_items),
            }
        }
        Self::Items(items).optimized()
    }

    /// Combine this query with others, covering all of them (OR).
    pub fn merge(self, others: impl IntoIterator<Item = Query>) -> Self {
        Self::combine(std::iter::once(self).chain(others))
    }

    /// Reduce this query to an equivalent, smaller form.
    ///
    /// Items with the same tags are merged, items covered by another item are
    /// dropped.
    fn optimized(self) -> Self {
        match self {
            Self::All => Self::All,
            Self::Items(items) => {
                // Merge items with identical tags by unioning their types.
                let mut by_tags: BTreeMap<Vec<String>, BTreeSet<String>> = BTreeMap::new();
                for item in items {
                    if item.types.is_empty() && item.tags.is_empty() {
                        return Self::All;
                    }
                    let types = by_tags.entry(item.tags.clone()).or_default();
                    if item.types.is_empty() {
                        // Tags-only item matches any event with these tags.
                        types.insert(String::new());
                    } else {
                        types.extend(item.types);
                    }
                }

                let mut items: Vec<QueryItem> = by_tags
                    .into_iter()
                    .map(|(tags, types)| {
                        // A wildcard type subsumes all typed items with the same tags.
                        let wildcard = types.contains("");
                        let types = if wildcard {
                            vec![]
                        } else {
                            let mut types: Vec<_> = types.into_iter().collect();
                            types.sort();
                            types
                        };
                        QueryItem { types, tags }
                    })
                    .collect();

                // Keep only items not covered by another kept item.
                let mut kept: Vec<QueryItem> = vec![];
                for item in items.drain(..) {
                    let dominated = kept.iter().any(|other| covers(other, &item));
                    if !dominated {
                        kept.retain(|other| !covers(&item, other));
                        kept.push(item);
                    }
                }
                Self::Items(kept)
            }
        }
    }
}

/// Whether `a` covers `b`: an event matching `b` also matches `a`.
fn covers(a: &QueryItem, b: &QueryItem) -> bool {
    let types_cover = a.types.is_empty() || {
        !b.types.is_empty() && b.types.iter().all(|t| a.types.contains(t))
    };
    let tags_cover = a.tags.iter().all(|t| b.tags.contains(t));
    types_cover && tags_cover
}

/// Condition that must be met when appending events.
///
/// Ensures that no events matching the query were stored after `after` since
/// the client built its decision model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppendCondition {
    /// Query that must not match any stored event.
    pub fail_if_events_match: Query,
    /// Events at or before this position are ignored while checking.
    pub after: Option<SequencePosition>,
}

impl AppendCondition {
    /// Create a new append condition.
    pub fn new(fail_if_events_match: Query, after: impl Into<Option<SequencePosition>>) -> Self {
        Self {
            fail_if_events_match,
            after: after.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::Event;

    fn event(event_type: &str, tags: Tags) -> Event {
        Event::new(event_type, serde_json::Value::Null, tags)
    }

    fn item(types: &[&str], tags: Tags) -> QueryItem {
        QueryItem::new(types.iter().map(|t| t.to_string()).collect(), tags)
    }

    #[test]
    fn all_query_matches_every_event() {
        let query = Query::all();

        assert!(query.matches(&event("created", vec![])));
        assert!(query.matches(&event("updated", vec!["product:1".into()])));
    }

    #[test]
    fn empty_items_query_matches_nothing() {
        let query = Query::from_items([]);

        assert!(!query.matches(&event("created", vec![])));
    }

    #[test]
    fn query_item_matches_one_of_the_types() {
        let item = item(&["created", "updated"], vec![]);

        assert!(item.matches(&event("created", vec![])));
        assert!(item.matches(&event("updated", vec![])));
        assert!(!item.matches(&event("deleted", vec![])));
    }

    #[test]
    fn query_item_matches_all_of_the_tags() {
        let item = item(&[], vec!["tag1".into(), "tag2".into()]);

        assert!(item.matches(&event("created", vec!["tag1".into(), "tag2".into()])));
        assert!(item.matches(&event(
            "created",
            vec!["tag1".into(), "tag2".into(), "tag3".into()]
        )));
        assert!(!item.matches(&event("created", vec!["tag1".into()])));
        assert!(!item.matches(&event("created", vec![])));
    }

    #[test]
    fn query_item_requires_type_and_tags_together() {
        let item = item(&["created"], vec!["tag1".into()]);

        assert!(item.matches(&event("created", vec!["tag1".into()])));
        assert!(!item.matches(&event("created", vec![])));
        assert!(!item.matches(&event("updated", vec!["tag1".into()])));
    }

    #[test]
    fn query_items_are_combined_with_or() {
        let query = Query::from_items([item(&["created"], vec![]), item(&["deleted"], vec![])]);

        assert!(query.matches(&event("created", vec![])));
        assert!(query.matches(&event("deleted", vec![])));
        assert!(!query.matches(&event("updated", vec![])));
    }

    #[test]
    fn spec_example_query_items_match_as_documented() {
        let query = Query::from_items([
            item(&["EventType1", "EventType2"], vec![]),
            item(&[], vec!["tag1".into(), "tag2".into()]),
            item(
                &["EventType2", "EventType3"],
                vec!["tag1".into(), "tag3".into()],
            ),
        ]);

        assert!(query.matches(&event("EventType1", vec![])));
        assert!(query.matches(&event("EventType2", vec!["unrelated".into()])));
        assert!(query.matches(&event("AnyType", vec!["tag1".into(), "tag2".into()])));
        assert!(query.matches(&event("EventType3", vec!["tag1".into(), "tag3".into()])));
        assert!(!query.matches(&event("EventType4", vec!["tag1".into()])));
        assert!(!query.matches(&event("EventType4", vec!["tag3".into()])));
    }

    #[test]
    fn query_item_with_no_constraints_matches_everything() {
        let item = item(&[], vec![]);

        assert!(item.matches(&event("created", vec![])));
        assert!(item.matches(&event("updated", vec!["tag1".into()])));
    }

    #[test]
    fn fingerprint_of_all_query() {
        assert_eq!(Query::all().fingerprint(), "all");
    }

    #[test]
    fn fingerprint_is_order_independent() {
        let a = Query::from_items([item(&["created"], vec![]), item(&["deleted"], vec![])]);
        let b = Query::from_items([item(&["deleted"], vec![]), item(&["created"], vec![])]);

        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_eq!(a.fingerprint(), "created|deleted");
    }

    #[test]
    fn fingerprint_sorts_types_and_tags() {
        let a = item(&["created", "updated"], vec!["tag2".into(), "tag1".into()]);
        let b = item(&["updated", "created"], vec!["tag1".into(), "tag2".into()]);

        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_eq!(a.fingerprint(), "created,updated:@tag1,tag2");
    }

    #[test]
    fn fingerprint_of_empty_item_matches_anything() {
        let query = Query::from_items([item(&[], vec![])]);

        assert_eq!(query.fingerprint(), "*");
    }

    #[test]
    fn fingerprint_of_tags_only_item() {
        let query = Query::from_items([item(&[], vec!["tag1".into(), "tag2".into()])]);

        assert_eq!(query.fingerprint(), "@tag1,tag2");
    }

    #[test]
    fn combine_merges_items_of_all_queries() {
        let combined = Query::combine([
            Query::from_items([item(&["created"], vec![])]),
            Query::from_items([item(&["deleted"], vec![])]),
        ]);

        assert!(combined.matches(&event("created", vec![])));
        assert!(combined.matches(&event("deleted", vec![])));
        assert!(!combined.matches(&event("updated", vec![])));
    }

    #[test]
    fn combine_with_all_matches_everything() {
        let combined = Query::combine([
            Query::from_items([item(&["created"], vec![])]),
            Query::all(),
            Query::from_items([item(&["deleted"], vec![])]),
        ]);

        assert_eq!(combined, Query::all());
        assert!(combined.matches(&event("whatever", vec![])));
    }

    #[test]
    fn combine_of_empty_input_is_empty_query() {
        let combined = Query::combine([]);

        assert_eq!(combined, Query::from_items([]));
        assert!(!combined.matches(&event("created", vec![])));
    }

    #[test]
    fn merge_combines_this_query_with_others() {
        let merged = Query::from_items([item(&["created"], vec![])])
            .merge([Query::from_items([item(&["deleted"], vec![])])]);

        assert!(merged.matches(&event("created", vec![])));
        assert!(merged.matches(&event("deleted", vec![])));
        assert!(!merged.matches(&event("updated", vec![])));
    }

    #[test]
    fn combine_merges_items_with_same_tags() {
        let combined = Query::combine([
            Query::from_items([item(&["created"], vec!["product:1".into()])]),
            Query::from_items([item(&["updated"], vec!["product:1".into()])]),
        ]);

        // Both types merged into a single item.
        assert_eq!(
            combined,
            Query::from_items([item(&["created", "updated"], vec!["product:1".into()])])
        );
        assert!(combined.matches(&event("updated", vec!["product:1".into()])));
        assert!(!combined.matches(&event("created", vec![])));
    }

    #[test]
    fn combine_deduplicates_items() {
        let combined = Query::combine([
            Query::from_items([item(&["created"], vec![])]),
            Query::from_items([item(&["created"], vec![])]),
        ]);

        assert_eq!(combined, Query::from_items([item(&["created"], vec![])]));
    }

    #[test]
    fn combine_drops_items_covered_by_another_item() {
        let combined = Query::combine([
            // `created` with no tags covers `created` with tag1.
            Query::from_items([item(&["created"], vec![])]),
            Query::from_items([item(&["created"], vec!["tag1".into()])]),
        ]);

        assert_eq!(combined, Query::from_items([item(&["created"], vec![])]));
    }

    #[test]
    fn combine_with_tags_only_item_covers_typed_items_with_same_tags() {
        let combined = Query::combine([
            Query::from_items([item(&[], vec!["product:1".into()])]),
            Query::from_items([item(&["created"], vec!["product:1".into()])]),
        ]);

        assert_eq!(
            combined,
            Query::from_items([item(&[], vec!["product:1".into()])])
        );
    }

    #[test]
    fn matches_event_type_evaluates_against_query_items() {
        let query = Query::from_items([
            item(&["UserRegistered", "UserProfileUpdated"], vec![]),
            item(&["UserDeleted"], vec!["tenant:1".into()]),
        ]);

        assert!(query.matches_event_type("UserRegistered"));
        assert!(query.matches_event_type("UserProfileUpdated"));
        assert!(query.matches_event_type("UserDeleted"));
        assert!(!query.matches_event_type("PasswordAuthenticated"));
        assert!(!query.matches_event_type("OrderCreated"));

        let all_query = Query::all();
        assert!(all_query.matches_event_type("PasswordAuthenticated"));
    }
}
