-- sqlfluff:dialect:postgres

-- ============================================
-- Appointment List (read endpoint support)
-- v2.list_fulfilled_appointments_by_patient: list a patient's FULFILLED
-- appointments, newest first, capped. Backs GET /v2/internal/appointments.
-- ============================================

DROP FUNCTION IF EXISTS v2.list_fulfilled_appointments_by_patient(integer, integer);

-- p_profile_id is nullable: NULL means "all profiles under the account".
-- A non-NULL value narrows to that single patient profile.
-- LIMIT 50 is a hard cap (no pagination yet); ORDER BY appointment_start DESC
-- keeps the cap deterministic (newest visits first).
CREATE OR REPLACE FUNCTION v2.list_fulfilled_appointments_by_patient(
    p_account_id integer,
    p_profile_id integer
)
RETURNS TABLE (
    booking_id varchar(20),
    appointment_start timestamptz,
    appointment_end timestamptz,
    doctor_account_id integer,
    doctor_profile_id integer
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        r.booking_id,
        r.appointment_start,
        r.appointment_end,
        r.doctor_account_id,
        r.doctor_profile_id
    FROM v2.reservation r
    INNER JOIN v2.appointment a ON a.appointment_id = r.booking_id
    WHERE r.patient_account_id = p_account_id
      AND (p_profile_id IS NULL OR r.patient_profile_id = p_profile_id)
      AND a.appointment_status = 'FULFILLED'::v2.fhir_appointment_status_enum
    ORDER BY r.appointment_start DESC
    LIMIT 50;
END;
$$ LANGUAGE plpgsql;
