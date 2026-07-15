-- sqlfluff:dialect:postgres

-- ============================================
-- Add prescription_id column to doctor_summary_note
-- ============================================
ALTER TABLE v2.doctor_summary_note
ADD COLUMN IF NOT EXISTS prescription_id bigint;

-- ============================================
-- Add unique constraint on appointment_id for upsert
-- ============================================
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'uk_doctor_summary_note_appointment_id'
    ) THEN
        ALTER TABLE v2.doctor_summary_note
        ADD CONSTRAINT uk_doctor_summary_note_appointment_id UNIQUE (appointment_id);
    END IF;
END $$;

-- ============================================
-- Create function: create_if_not_existing_summary_note
-- Creates a summary note if one doesn't already exist for the appointment
-- Returns: summary_note_id if created, NULL if already exists
-- ============================================
DROP FUNCTION IF EXISTS v2.create_if_not_existing_summary_note CASCADE;
CREATE OR REPLACE FUNCTION v2.create_if_not_existing_summary_note(
    p_appointment_id varchar(20),
    p_encrypted_data text,
    p_encrypted_data_type varchar(120),
    p_note_to_staff text,
    p_icd10_codes jsonb,
    p_prescription_id bigint DEFAULT NULL
) RETURNS bigint AS $$
DECLARE
    v_summary_note_id bigint;
BEGIN
    INSERT INTO v2.doctor_summary_note (
        appointment_id,
        encrypted_data,
        encrypted_data_type,
        note_to_staff,
        icd10_codes,
        prescription_id
    ) VALUES (
        p_appointment_id,
        p_encrypted_data,
        p_encrypted_data_type,
        p_note_to_staff,
        p_icd10_codes,
        p_prescription_id
    )
    ON CONFLICT (appointment_id) DO NOTHING
    RETURNING summary_note_id INTO v_summary_note_id;

    RETURN v_summary_note_id;
END;
$$ LANGUAGE plpgsql;
