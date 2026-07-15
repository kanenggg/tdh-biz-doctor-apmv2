-- sqlfluff:dialect:postgres

-- ============================================
-- Summary submit fulfils the appointment.
-- v2.create_if_not_existing_summary_note now also transitions the appointment
-- to FULFILLED in the same transaction as the note upsert. Submitting the
-- doctor summary is the terminal "consultation done" action
-- (legacy CONSULTATION_DONE -> FULFILLED), so the status must move with the note.
--
-- The transition is UNCONDITIONAL for now (no from-status guard): it mirrors
-- v2.end_active_session, which also sets FULFILLED without checking the prior
-- state. A from-status guard (BOOKED -> FULFILLED only) is a deliberate follow-up.
--
-- This keeps the read-side invariant intact: v2 consultation-summary reads gate
-- on appointment_status = 'FULFILLED', and a FULFILLED row is expected to have a
-- summary note. Doing the UPDATE in the same function/transaction as the note
-- upsert means the two can never diverge.
-- ============================================
DROP FUNCTION IF EXISTS v2.create_if_not_existing_summary_note CASCADE;
CREATE OR REPLACE FUNCTION v2.create_if_not_existing_summary_note(
    p_appointment_id varchar(20),
    p_encrypted_data text,
    p_encrypted_data_type varchar(120),
    p_note_to_staff text,
    p_icd10_codes jsonb,
    p_prescription_id bigint DEFAULT NULL
) RETURNS jsonb AS $$
DECLARE
    v_summary_note_id bigint;
    v_patient_account_id integer;
    v_patient_profile_id integer;
    v_biz_unit_id integer;
    v_biz_center_id integer;
    v_tenant_id integer;
    v_created boolean := true;
    v_is_follow_up boolean;
BEGIN
    SELECT parent_appointment_id IS NOT NULL INTO v_is_follow_up
    FROM v2.appointment
    WHERE appointment_id = p_appointment_id;

    IF v_is_follow_up THEN
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
        ON CONFLICT (appointment_id) DO UPDATE SET
            encrypted_data = EXCLUDED.encrypted_data,
            encrypted_data_type = EXCLUDED.encrypted_data_type,
            note_to_staff = EXCLUDED.note_to_staff,
            icd10_codes = EXCLUDED.icd10_codes,
            prescription_id = EXCLUDED.prescription_id
        RETURNING summary_note_id INTO v_summary_note_id;

        IF v_summary_note_id IS NULL THEN
            v_created := false;
        END IF;
    ELSE
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

        IF v_summary_note_id IS NULL THEN
            v_created := false;
        END IF;
    END IF;

    IF v_summary_note_id IS NULL THEN
        SELECT dsn.summary_note_id INTO v_summary_note_id
        FROM v2.doctor_summary_note dsn
        WHERE dsn.appointment_id = p_appointment_id;
    END IF;

    -- Terminal transition: submitting the summary fulfils the appointment.
    UPDATE v2.appointment
    SET appointment_status = 'FULFILLED'::v2.fhir_appointment_status_enum,
        modified_at = NOW()
    WHERE appointment_id = p_appointment_id;

    SELECT
        r.patient_account_id,
        r.patient_profile_id,
        r.biz_unit_id,
        r.biz_center_id,
        r.tenant_id
    INTO
        v_patient_account_id,
        v_patient_profile_id,
        v_biz_unit_id,
        v_biz_center_id,
        v_tenant_id
    FROM v2.reservation r
    WHERE r.booking_id = p_appointment_id;

    RETURN jsonb_build_object(
        'created', v_created,
        'summaryNoteId', COALESCE(v_summary_note_id, 0),
        'patientAccountId', COALESCE(v_patient_account_id, 0),
        'userProfileId', COALESCE(v_patient_profile_id, 0),
        'tenantId', COALESCE(v_tenant_id, 1),
        'bizUnitId', COALESCE(v_biz_unit_id, 0),
        'bizCenterId', COALESCE(v_biz_center_id, 0)
    );
END;
$$ LANGUAGE plpgsql;
