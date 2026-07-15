-- sqlfluff:dialect:postgres

-- Replace create_reservation with DB-atomic doctor/time overlap protection.
-- `reserved_until` is the hold/payment deadline (now + TTL), while
-- `appointment_end` is derived from the requested timeslot duration.
DROP FUNCTION IF EXISTS v2.create_reservation(
    integer,
    integer,
    integer,
    integer,
    integer,
    integer,
    integer,
    integer,
    v2.booking_type_enum,
    v2.consultation_type_enum,
    timestamptz,
    integer
);

CREATE OR REPLACE FUNCTION v2.create_reservation(
    p_patient_account_id integer,
    p_patient_profile_id integer,
    p_doctor_id integer,
    p_doctor_account_id integer,
    p_doctor_profile_id integer,
    p_biz_unit_id integer,
    p_biz_center_id integer,
    p_tenant_id integer,
    p_booking_type v2.booking_type_enum,
    p_consultation_channel v2.consultation_type_enum,
    p_reserved_from timestamptz,
    p_ttl_seconds integer,
    p_duration_seconds bigint
) RETURNS TABLE (
    booking_id varchar(20)
) AS $$
DECLARE
    v_reservation_id varchar(20);
    v_appointment_end timestamptz;
    v_reserved_until timestamptz;
BEGIN
    IF p_duration_seconds <= 0 THEN
        RAISE EXCEPTION 'Invalid reservation duration seconds: %', p_duration_seconds
            USING ERRCODE = '22023';
    END IF;

    v_appointment_end := p_reserved_from + (p_duration_seconds || ' seconds')::interval;
    v_reserved_until := NOW() + (p_ttl_seconds || ' seconds')::interval;

    -- Serialize reservation creation by doctor profile so two concurrent requests
    -- cannot both pass the overlap check for the same doctor.
    PERFORM pg_advisory_xact_lock(p_doctor_profile_id::bigint);

    IF EXISTS (
        SELECT 1
        FROM v2.reservation r
        WHERE r.doctor_profile_id = p_doctor_profile_id
          AND r.deleted_at IS NULL
          AND r.reservation_status IN (
              'RESERVED'::v2.reservation_status_enum,
              'CONFIRMED'::v2.reservation_status_enum
          )
          AND (
              r.reservation_status = 'CONFIRMED'::v2.reservation_status_enum
              OR r.reserved_until > NOW()
          )
          AND tstzrange(r.appointment_start, r.appointment_end, '[)')
              && tstzrange(p_reserved_from, v_appointment_end, '[)')
    ) THEN
        RAISE EXCEPTION 'Timeslot is already reserved for doctor_profile_id=%', p_doctor_profile_id
            USING ERRCODE = '23P01';
    END IF;

    v_reservation_id := v2.generate_booking_id();

    INSERT INTO v2.reservation (
        booking_id,
        patient_account_id,
        patient_profile_id,
        doctor_id,
        doctor_account_id,
        doctor_profile_id,
        biz_unit_id,
        biz_center_id,
        tenant_id,
        reservation_status,
        reserved_until,
        booking_type,
        consultation_channel,
        appointment_start,
        appointment_end
    ) VALUES (
        v_reservation_id,
        p_patient_account_id,
        p_patient_profile_id,
        p_doctor_id,
        p_doctor_account_id,
        p_doctor_profile_id,
        p_biz_unit_id,
        p_biz_center_id,
        p_tenant_id,
        'RESERVED'::v2.reservation_status_enum,
        v_reserved_until,
        p_booking_type,
        p_consultation_channel,
        p_reserved_from,
        v_appointment_end
    );

    RETURN QUERY
    SELECT r.booking_id
    FROM v2.reservation r
    WHERE r.booking_id = v_reservation_id;
END;
$$ LANGUAGE plpgsql;
