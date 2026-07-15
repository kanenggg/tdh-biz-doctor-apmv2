-- sqlfluff:dialect:postgres

-- ============================================
-- patient_prescreen table
-- ============================================
CREATE TABLE IF NOT EXISTS v2.patient_prescreen (
    prescreen_id serial PRIMARY KEY,
    booking_id varchar(20) NOT NULL,

    prescreen_data text NOT NULL,
    prescreen_data_type varchar(255) NOT NULL,

    user_account_id integer NOT NULL,
    user_profile_id integer NOT NULL,

    created_at timestamptz NOT NULL DEFAULT now(),
    modified_at timestamptz NOT NULL DEFAULT now()
);

-- Foreign key: patient_prescreen references reservation
-- ALTER TABLE v2.patient_prescreen
-- ADD CONSTRAINT fk_patient_prescreen_booking_id
-- FOREIGN KEY (booking_id)
-- REFERENCES v2.reservation (booking_id);

-- Index for patient_prescreen
CREATE INDEX IF NOT EXISTS idx_patient_prescreen_booking_id
ON v2.patient_prescreen (booking_id);

-- Trigger for patient_prescreen
CREATE TRIGGER update_patient_prescreen_modified_at
BEFORE UPDATE ON v2.patient_prescreen
FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();
