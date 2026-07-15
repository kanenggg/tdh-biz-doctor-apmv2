-- sqlfluff:dialect:postgres

-- Generic durable event outbox for APM consultation/domain events.
-- API services enqueue the JSON payload before attempting direct publish. The
-- background dispatcher retries pending/expired processing rows until published.
CREATE TABLE IF NOT EXISTS v2.event_outbox (
    event_id uuid PRIMARY KEY,
    topic varchar(255) NOT NULL,
    event_type varchar(255) NOT NULL,
    aggregate_id varchar(255),
    payload jsonb NOT NULL,
    publication_status varchar(32) NOT NULL DEFAULT 'PENDING',
    retry_count integer NOT NULL DEFAULT 0,
    locked_until timestamptz,
    last_error text,
    created_at timestamptz NOT NULL DEFAULT now(),
    published_at timestamptz,
    modified_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_event_outbox_dispatch
ON v2.event_outbox (publication_status, locked_until, created_at);

CREATE INDEX IF NOT EXISTS idx_event_outbox_aggregate
ON v2.event_outbox (aggregate_id, event_type);

CREATE TRIGGER update_event_outbox_modified_at
BEFORE UPDATE ON v2.event_outbox
FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();
