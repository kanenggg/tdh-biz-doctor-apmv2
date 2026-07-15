-- sqlfluff:dialect:postgres

-- Make session initialization and doctor end-session event publication idempotent.
-- v2.upsert_session_info returns true only for the insert winner; callers use
-- this to publish SessionCreated once under concurrent first access.
DROP FUNCTION IF EXISTS v2.upsert_session_info(varchar, jsonb) CASCADE;
CREATE OR REPLACE FUNCTION v2.upsert_session_info(
    p_appointment_id varchar(20),
    p_session_data jsonb
) RETURNS boolean AS $$
DECLARE
    v_inserted boolean := false;
BEGIN
    INSERT INTO v2.session_info (
        appointment_id,
        session_provider,
        session_data
    )
    VALUES (
        p_appointment_id,
        'TWILIO'::text,
        p_session_data
    )
    ON CONFLICT (appointment_id) DO NOTHING
    RETURNING true INTO v_inserted;

    RETURN COALESCE(v_inserted, false);
END;
$$ LANGUAGE plpgsql;

-- Return 0 once an appointment has already left active statuses so repeated
-- end-session calls cannot publish duplicate/conflicting SessionTerminated.
CREATE OR REPLACE FUNCTION v2.end_active_session(
    p_appointment_id varchar(20),
    p_doctor_profile_id bigint
) returns bigint
language plpgsql
as $$
DECLARE
    v_rows_affected bigint;
BEGIN
    UPDATE v2.appointment a
    SET appointment_status = 'CONSULTATION_DONE'::v2.fhir_appointment_status_enum,
        modified_at = NOW()
    WHERE a.appointment_id = p_appointment_id
      AND a.appointment_status IN (
          'BOOKED'::v2.fhir_appointment_status_enum,
          'ARRIVED'::v2.fhir_appointment_status_enum
      )
      AND EXISTS (
          SELECT 1
          FROM v2.reservation r
          WHERE r.booking_id = a.booking_id
            AND r.doctor_profile_id = p_doctor_profile_id
      );
    GET DIAGNOSTICS v_rows_affected = ROW_COUNT;
    RETURN v_rows_affected;
END;
$$;
