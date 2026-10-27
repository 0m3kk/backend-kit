-- Shared checkpoint table used by catch-up projections.
-- Each projection stores its last processed event position here.

CREATE TABLE IF NOT EXISTS projection_checkpoints (
    view_name           TEXT PRIMARY KEY,
    last_event_position BIGINT NOT NULL DEFAULT 0,
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
