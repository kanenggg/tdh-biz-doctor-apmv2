-- sqlfluff:dialect:postgres

-- ============================================
-- appointment_cancellation table
-- ============================================
CREATE TABLE IF NOT EXISTS v2.appointment_cancellation (
    appointment_id bigint NOT NULL,
    cancel_reason jsonb,
    created_at timestamptz
);

-- Index for appointment_cancellation
CREATE INDEX IF NOT EXISTS idx_appointment_cancellation_appointment_id
ON v2.appointment_cancellation (appointment_id);
