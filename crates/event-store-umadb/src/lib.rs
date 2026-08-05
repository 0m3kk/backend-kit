use async_trait::async_trait;
use std::sync::Arc;

pub use event_sourcing::*;
pub use umadb_client::AsyncUmaDbClient;
pub use umadb_dcb::*;
use uuid::Uuid;

/// An UmaDB implementation of [`EventStore`].
#[derive(Clone)]
pub struct UmaDBEventStore {
    client: Arc<AsyncUmaDbClient>,
}

impl UmaDBEventStore {
    /// Creates a store wrapping an existing [`AsyncUmaDbClient`].
    pub fn new(client: AsyncUmaDbClient) -> Self {
        Self {
            client: Arc::new(client),
        }
    }

    /// Connects to a remote UmaDB server gRPC endpoint.
    pub async fn connect(
        url: impl Into<String>,
        api_key: Option<String>,
    ) -> Result<Self, DcbError> {
        let client = AsyncUmaDbClient::connect(url.into(), None, None, api_key).await?;
        Ok(Self::new(client))
    }

    /// Returns a reference to the underlying [`AsyncUmaDbClient`].
    pub fn client(&self) -> &AsyncUmaDbClient {
        &self.client
    }
}

#[async_trait]
impl EventStore for UmaDBEventStore {
    async fn read(&self, query: &Query, options: ReadOptions) -> EventStream {
        let dcb_query = if matches!(query, Query::All) {
            None
        } else {
            Some(convert_query(query))
        };

        let start = options.after.map(|p| p.value());
        let backwards = matches!(options.direction, Direction::Backward);
        let limit = options.limit.map(|l| l as u32);

        let client = self.client.clone();

        let stream = async_stream::stream! {
            let response_res = client.read(dcb_query, start, backwards, limit).await;
            match response_res {
                Ok(mut response) => {
                    loop {
                        match response.next_batch().await {
                            Ok(batch) => {
                                if batch.is_empty() {
                                    break;
                                }
                                for seq_event in batch {
                                    yield Ok(convert_dcb_sequenced_event(seq_event));
                                }
                            }
                            Err(e) => {
                                yield Err(ReadError::StoreError(e.to_string()));
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    yield Err(ReadError::StoreError(e.to_string()));
                }
            }
        };

        Box::pin(stream)
    }

    async fn append(
        &self,
        events: Vec<Event>,
        condition: Option<AppendCondition>,
    ) -> Result<Vec<SequencedEvent>, AppendError> {
        if events.is_empty() {
            return Err(AppendError::EmptyBatch);
        }

        let total_events = events.len();
        let dcb_events: Vec<DcbEvent> = events.into_iter().map(convert_event).collect();
        let dcb_cond = condition.as_ref().map(convert_append_condition);

        let last_position = self
            .client
            .append(dcb_events.clone(), dcb_cond, None)
            .await
            .map_err(|e| match e {
                DcbError::IntegrityError(_) => {
                    let first_evt = dcb_events.first().cloned().unwrap_or_default();
                    let dummy_seq = SequencedEvent::new(
                        SequencePosition::new(0),
                        0,
                        convert_dcb_event(first_evt),
                    );

                    AppendError::Conflict {
                        condition: condition
                            .clone()
                            .unwrap_or_else(|| AppendCondition::new(Query::item(QueryItem::new()))),
                        conflicting_event: dummy_seq,
                    }
                }
                other => AppendError::StoreError(other.to_string()),
            })?;

        let start_pos = last_position.saturating_sub(total_events as u64) + 1;
        let appended = dcb_events
            .into_iter()
            .enumerate()
            .map(|(idx, dcb_evt)| {
                let pos = start_pos + idx as u64;
                let evt = convert_dcb_event(dcb_evt);
                SequencedEvent::new(SequencePosition::new(pos), 0, evt)
            })
            .collect();

        Ok(appended)
    }
}

/// Converts an `event_sourcing::Event` into a `umadb_dcb::DcbEvent`.
fn convert_event(event: Event) -> DcbEvent {
    let uuid = Uuid::parse_str(event.id.as_str()).ok();
    let data_bytes = serde_json::to_vec(&event.data).unwrap_or_default();
    let tags: Vec<String> = event.tags.into_iter().map(|t| t.0).collect();

    let mut dcb_evt = DcbEvent::new()
        .event_type(event.event_type.0)
        .data(data_bytes)
        .tags(tags);

    if let Some(u) = uuid {
        dcb_evt = dcb_evt.uuid(u);
    } else {
        dcb_evt = dcb_evt.metadata_entry("id", event.id.as_str().to_string());
    }

    if let Some(meta) = event.metadata {
        let meta_str = meta.to_string();
        dcb_evt = dcb_evt.metadata_entry("_metadata_json", meta_str);
    }

    dcb_evt
}

/// Converts a `umadb_dcb::DcbEvent` into an `event_sourcing::Event`.
fn convert_dcb_event(dcb_evt: DcbEvent) -> Event {
    let id_str = if let Some(u) = dcb_evt.uuid {
        u.to_string()
    } else {
        dcb_evt
            .metadata
            .iter()
            .find(|(k, _)| k == "id")
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string())
    };

    let data_val: serde_json::Value =
        serde_json::from_slice(&dcb_evt.data).unwrap_or(serde_json::Value::Null);

    let tags = dcb_evt.tags.into_iter().map(Tag::new).collect();

    let mut event = Event::new(id_str, dcb_evt.event_type, data_val, tags);

    if let Some((_, meta_str)) = dcb_evt.metadata.iter().find(|(k, _)| k == "_metadata_json")
        && let Ok(meta_json) = serde_json::from_str::<serde_json::Value>(meta_str)
    {
        event = event.with_metadata(meta_json);
    }

    event
}

/// Converts a `umadb_dcb::DcbSequencedEvent` into an `event_sourcing::SequencedEvent`.
fn convert_dcb_sequenced_event(dcb_seq: DcbSequencedEvent) -> SequencedEvent {
    let evt = convert_dcb_event(dcb_seq.event);
    SequencedEvent::new(SequencePosition::new(dcb_seq.position), 0, evt)
}

/// Converts an `event_sourcing::Query` into a `umadb_dcb::DcbQuery`.
fn convert_query(query: &Query) -> DcbQuery {
    match query {
        Query::All => DcbQuery::new(),
        Query::Items { items } => {
            let dcb_items: Vec<DcbQueryItem> = items
                .iter()
                .map(|item| {
                    let types: Vec<String> = item.types.iter().map(|t| t.0.clone()).collect();
                    let tags: Vec<String> = item.tags.iter().map(|t| t.0.clone()).collect();
                    DcbQueryItem::new().types(types).tags(tags)
                })
                .collect();
            DcbQuery::with_items(dcb_items)
        }
    }
}

/// Converts an `event_sourcing::AppendCondition` into a `umadb_dcb::DcbAppendCondition`.
fn convert_append_condition(cond: &AppendCondition) -> DcbAppendCondition {
    let dcb_query = convert_query(&cond.fail_if_events_match);
    let after = cond.after.map(|p| p.value());
    DcbAppendCondition::new(dcb_query).after(after)
}

#[cfg(test)]
mod unit_tests;
