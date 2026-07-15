-- sqlfluff:dialect:postgres

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
