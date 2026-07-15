-- sqlfluff:dialect:postgres

-- ============================================
-- Common Functions and Sequences
-- ============================================

-- Sequence for generating booking IDs
CREATE SEQUENCE IF NOT EXISTS v2.reservation_booking_id_seq START WITH 100000;

-- Modified at trigger function
CREATE OR REPLACE FUNCTION v2.update_modified_at_column()
RETURNS trigger AS $$
BEGIN
    NEW.modified_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
