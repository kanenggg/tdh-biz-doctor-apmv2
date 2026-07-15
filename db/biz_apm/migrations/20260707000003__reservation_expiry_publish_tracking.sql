-- sqlfluff:dialect:postgres

-- Durable-ish expiry event bookkeeping for the background reservation release worker.
-- A stale claim may be retried so a worker crash does not permanently suppress
-- ReservationExpired publication.
ALTER TABLE v2.reservation
ADD COLUMN IF NOT EXISTS expired_at timestamptz,
ADD COLUMN IF NOT EXISTS expiration_event_claimed_at timestamptz,
ADD COLUMN IF NOT EXISTS expiration_event_published_at timestamptz;

CREATE INDEX IF NOT EXISTS idx_reservation_expiry_due
ON v2.reservation (reserved_until, reservation_status)
WHERE deleted_at IS NULL AND expiration_event_published_at IS NULL;
