-- sqlfluff:dialect:postgres

DROP FUNCTION IF EXISTS v2.get_booking_state(varchar);
CREATE OR REPLACE FUNCTION v2.get_booking_state(
    p_booking_id varchar(20)
) RETURNS TABLE (
    booking_id varchar(20),
    patient_account_id integer,
    patient_profile_id integer,
    tenant_id integer,
    doctor_id integer,
    biz_unit_id integer,
    reservation_status text,
    appointment_status text,
    reserved_until bigint,
    appointment_start bigint,
    appointment_end bigint
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        r.booking_id,
        r.patient_account_id,
        r.patient_profile_id,
        r.tenant_id,
        r.doctor_id,
        COALESCE(r.biz_unit_id, 0) AS biz_unit_id,
        r.reservation_status::text AS reservation_status,
        a.appointment_status::text AS appointment_status,
        FLOOR(EXTRACT(EPOCH FROM r.reserved_until))::bigint AS reserved_until,
        FLOOR(EXTRACT(EPOCH FROM r.appointment_start))::bigint AS appointment_start,
        FLOOR(EXTRACT(EPOCH FROM r.appointment_end))::bigint AS appointment_end
    FROM v2.reservation r
    LEFT JOIN v2.appointment a ON a.appointment_id = r.booking_id
    WHERE r.booking_id = p_booking_id
      AND r.deleted_at IS NULL;
END;
$$ LANGUAGE plpgsql STABLE;

DROP FUNCTION IF EXISTS v2.cancel_reserved_booking(varchar);
CREATE OR REPLACE FUNCTION v2.cancel_reserved_booking(
    p_booking_id varchar(20)
) RETURNS TABLE (
    booking_id varchar(20),
    patient_account_id integer,
    patient_profile_id integer,
    tenant_id integer,
    doctor_id integer,
    biz_unit_id integer,
    reservation_status text,
    appointment_status text,
    cancelled_at bigint,
    state_changed boolean
) AS $$
DECLARE
    v_reservation RECORD;
    v_appointment_status text;
    v_state_changed boolean := false;
BEGIN
    SELECT * INTO v_reservation
    FROM v2.reservation r
    WHERE r.booking_id = p_booking_id
      AND r.deleted_at IS NULL
    FOR UPDATE;

    IF v_reservation IS NULL THEN
        RETURN;
    END IF;

    SELECT a.appointment_status::text INTO v_appointment_status
    FROM v2.appointment a
    WHERE a.appointment_id = p_booking_id
    FOR UPDATE;

    IF v_appointment_status IS NOT NULL
       AND v_appointment_status NOT IN ('PENDING', 'CANCELLED') THEN
        RAISE EXCEPTION 'Cannot cancel reserved booking_id=%, appointment status is %',
            p_booking_id, v_appointment_status;
    END IF;

    IF v_reservation.reservation_status = 'RESERVED'::v2.reservation_status_enum THEN
        UPDATE v2.reservation
        SET reservation_status = 'CANCELLED'::v2.reservation_status_enum,
            cancelled_at = NOW(),
            modified_at = NOW()
        WHERE reservation.booking_id = p_booking_id;
        v_state_changed := true;
    END IF;

    RETURN QUERY
    SELECT
        r.booking_id,
        r.patient_account_id,
        r.patient_profile_id,
        r.tenant_id,
        r.doctor_id,
        COALESCE(r.biz_unit_id, 0) AS biz_unit_id,
        r.reservation_status::text AS reservation_status,
        a.appointment_status::text AS appointment_status,
        FLOOR(EXTRACT(EPOCH FROM COALESCE(r.cancelled_at, NOW())))::bigint AS cancelled_at,
        v_state_changed AS state_changed
    FROM v2.reservation r
    LEFT JOIN v2.appointment a ON a.appointment_id = r.booking_id
    WHERE r.booking_id = p_booking_id;
END;
$$ LANGUAGE plpgsql;
