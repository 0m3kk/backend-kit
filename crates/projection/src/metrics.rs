//! OpenTelemetry metrics emitted by the projection supervisor.

use event_store::{SequencePosition, SequencedEvent};
use telemetry::{Counter, FloatGauge, ValueGauge};

/// Number of events applied by this process, labeled by view.
pub const PROCESSED_TOTAL: &str = "projection_processed_total";

/// Number of failed catch-up attempts, labeled by view.
pub const ERRORS_TOTAL: &str = "projection_errors_total";

/// Highest applied event position, labeled by view.
pub const LAST_POSITION: &str = "projection_last_position";

/// Age in seconds of the oldest event in the last applied batch, labeled by
/// view. Zero when the view is caught up.
pub const LAG_SECONDS: &str = "projection_lag_seconds";

const METER_SCOPE: &str = "projection";

#[derive(Clone)]
pub(crate) struct ProjectionMetrics {
    processed_total: Counter,
    errors_total: Counter,
    last_position: ValueGauge,
    lag_seconds: FloatGauge,
}

impl ProjectionMetrics {
    pub(crate) fn new() -> Self {
        Self {
            processed_total: Counter::with_scope(METER_SCOPE, PROCESSED_TOTAL),
            errors_total: Counter::with_scope(METER_SCOPE, ERRORS_TOTAL),
            last_position: ValueGauge::with_scope(METER_SCOPE, LAST_POSITION),
            lag_seconds: FloatGauge::with_scope(METER_SCOPE, LAG_SECONDS),
        }
    }

    pub(crate) fn record_success(
        &self,
        view: &'static str,
        position: SequencePosition,
        processed: u64,
        lag_seconds: Option<f64>,
    ) {
        self.processed_total.add(processed, [("view", view)]);
        self.last_position.record_with(position, [("view", view)]);
        self.lag_seconds
            .record_with(lag_seconds.unwrap_or(0.0), [("view", view)]);
    }

    pub(crate) fn record_error(&self, view: &'static str) {
        self.errors_total.inc_with([("view", view)]);
    }
}

/// Returns the age of `event` in seconds, using its typed metadata timestamp.
///
/// Negative ages (clock skew between the writer and the worker) are clamped to
/// zero. Events without typed metadata do not report lag.
pub(crate) fn event_age_seconds(event: &SequencedEvent) -> Option<f64> {
    let metadata = event.event.typed_metadata().ok()?;
    let age = (time::OffsetDateTime::now_utc() - metadata.timestamp).as_seconds_f64();
    Some(age.max(0.0))
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use event_store::{Event, EventMetadata};
    use serde_json::json;

    use super::*;

    fn event_with_timestamp(
        timestamp: time::OffsetDateTime,
    ) -> Result<SequencedEvent, serde_json::Error> {
        let event = Event::new("test.event", json!({}), Vec::new())
            .with_typed_metadata(&EventMetadata::new(timestamp))?;
        Ok(SequencedEvent { event, position: 1 })
    }

    #[test]
    fn caches_up_lag_from_event_metadata() -> Result<(), Box<dyn Error>> {
        let event =
            event_with_timestamp(time::OffsetDateTime::now_utc() - time::Duration::seconds(2))?;
        let age = event_age_seconds(&event).unwrap_or_default();
        assert!((1.9..3.0).contains(&age));
        Ok(())
    }

    #[test]
    fn clamps_future_timestamps_to_zero() -> Result<(), Box<dyn Error>> {
        let event =
            event_with_timestamp(time::OffsetDateTime::now_utc() + time::Duration::seconds(5))?;
        assert_eq!(event_age_seconds(&event), Some(0.0));
        Ok(())
    }

    #[test]
    fn missing_metadata_has_no_lag() {
        let event = SequencedEvent {
            event: Event::new("test.event", json!({}), Vec::new()),
            position: 1,
        };
        assert_eq!(event_age_seconds(&event), None);
    }
}
