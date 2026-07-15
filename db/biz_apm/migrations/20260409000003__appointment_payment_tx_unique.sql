-- sqlfluff:dialect:postgres

-- ============================================
-- Add UNIQUE constraint on appointment_payment_transaction.appointment_id
-- ============================================
-- The table had no uniqueness constraint on appointment_id, which:
-- 1. Allowed duplicate payment rows per appointment (data integrity risk).
-- 2. Made v2.create_confirmed_appointment's ON CONFLICT (appointment_id)
--    clause a latent bug — PG only validates ON CONFLICT targets at
--    execution time, so the function was creatable but would fail
--    the first time the conflict path fired.
-- 3. Broke the v2.get_appointment_detail read path because
--    fetch_optional assumes at most one row.
--
-- This migration deduplicates any accidentally-existing duplicates
-- (keeping the most recently modified row per appointment_id) and
-- then adds the constraint.

WITH ranked AS (
    SELECT
        ctid,
        appointment_id,
        row_number()
            OVER (PARTITION BY appointment_id ORDER BY modified_at DESC)
            AS rn
    FROM v2.appointment_payment_transaction
)
DELETE FROM v2.appointment_payment_transaction t
USING ranked
WHERE t.ctid = ranked.ctid AND ranked.rn > 1;

ALTER TABLE v2.appointment_payment_transaction
    ADD CONSTRAINT appointment_payment_transaction_appointment_id_key
    UNIQUE (appointment_id);
