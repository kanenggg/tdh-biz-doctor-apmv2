-- sqlfluff:dialect:postgres

-- ============================================
-- doctor_summary_note table
-- ============================================
CREATE TABLE IF NOT EXISTS v2.doctor_summary_note (
    summary_note_id serial PRIMARY KEY,
    appointment_id varchar(20) NOT NULL,
    encrypted_data text NOT NULL,
    encrypted_data_type varchar(120) NOT NULL DEFAULT 'DoctorSummaryNoteV1',
    note_to_staff text,
    icd10_codes jsonb NOT NULL DEFAULT '{}',
    tenant_id integer NOT NULL DEFAULT 1,

    created_at timestamptz NOT NULL DEFAULT now(),
    modified_at timestamptz NOT NULL DEFAULT now()
);

-- Foreign key: doctor_summary_note references appointment
-- ALTER TABLE v2.doctor_summary_note
-- ADD CONSTRAINT fk_doctor_summary_note_appointment_id
-- FOREIGN KEY (appointment_id) REFERENCES v2.appointment (appointment_id);

-- Indexes for doctor_summary_note
CREATE INDEX IF NOT EXISTS idx_doctor_summary_note_appointment_id
ON v2.doctor_summary_note (appointment_id);
CREATE INDEX IF NOT EXISTS idx_doctor_summary_note_tenant_id
ON v2.doctor_summary_note (tenant_id);
CREATE INDEX IF NOT EXISTS idx_doctor_summary_note_icd10_codes
ON v2.doctor_summary_note USING gin (icd10_codes);

-- Trigger for doctor_summary_note
CREATE TRIGGER update_doctor_summary_note_modified_at
BEFORE UPDATE ON v2.doctor_summary_note
FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();
