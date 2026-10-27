-- Emit a notification after an event is appended so projection workers can
-- catch up immediately instead of waiting for the next polling tick.
CREATE OR REPLACE FUNCTION event_store_notify_events_appended() RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('events_appended', NEW.event_type);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE TRIGGER event_store_events_appended
AFTER INSERT ON events
FOR EACH ROW
EXECUTE FUNCTION event_store_notify_events_appended();
