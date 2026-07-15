-- sqlfluff:dialect:postgres

-- ============================================
-- End Active Session
-- Updates appointment status to FULFILLED and returns rows affected
-- ============================================
DROP FUNCTION IF EXISTS v2.end_active_session CASCADE;
CREATE FUNCTION v2.end_active_session(
    p_appointment_id varchar(20),
    p_doctor_profile_id bigint
) returns bigint
language plpgsql
as $$
DECLARE
    v_rows_affected bigint;
BEGIN
    UPDATE v2.appointment a
    SET appointment_status = 'FULFILLED'::v2.fhir_appointment_status_enum,
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

-- ============================================
-- Cancel Appointment
-- Cancels an appointment by setting its status to CANCELLED
-- Returns rows affected
-- ============================================
DROP FUNCTION IF EXISTS v2.cancel_appointment CASCADE;
CREATE OR REPLACE FUNCTION v2.cancel_appointment(
    p_booking_id varchar(20),
    p_doctor_id bigint
) RETURNS bigint AS $$
DECLARE
    v_rows_affected bigint;
BEGIN
    UPDATE v2.appointment a
    SET appointment_status = 'CANCELLED'::v2.fhir_appointment_status_enum,
        modified_at = NOW()
    WHERE a.appointment_id = p_booking_id
      AND EXISTS (
          SELECT 1
          FROM v2.reservation r
          WHERE r.booking_id = a.appointment_id
            AND r.doctor_profile_id = p_doctor_id
            AND r.deleted_at IS NULL
      );
    GET DIAGNOSTICS v_rows_affected = ROW_COUNT;
    RETURN v_rows_affected;
END;
$$ LANGUAGE plpgsql;

-- ============================================
-- Get Consultation Session
-- Joins v2.reservation, v2.appointment, and v2.session_info tables to return unified session info
-- ============================================
DROP FUNCTION IF EXISTS v2.get_consultation_session CASCADE;
CREATE FUNCTION v2.get_consultation_session(
    p_appointment_id varchar(20), p_user_profile_id integer
)
RETURNS TABLE
(
    appointment_id varchar(20),
    session_provider_name text,
    session_data jsonb,
    appointment_status v2.fhir_appointment_status_enum,
    patient_profile_id bigint,
    doctor_profile_id bigint,
    consultation_start_time bigint,
    consultation_end_time bigint,
    consultation_channel v2.consultation_type_enum,
    payment_channels jsonb,
    is_facial_verified boolean
)
STABLE
LANGUAGE plpgsql
AS
$$
BEGIN
    RETURN QUERY
        SELECT a.appointment_id,
               COALESCE(si.session_provider, 'TWILIO')::text   as session_provider_name,
               si.session_data,
               a.appointment_status,
               r.patient_profile_id::bigint,
               r.doctor_profile_id::bigint,
               EXTRACT(EPOCH FROM r.appointment_start)::bigint as consultation_start_time,
               EXTRACT(EPOCH FROM r.appointment_end)::bigint   as consultation_end_time,
               r.consultation_channel,
               pay.payment_channels,
               CASE
                   WHEN r.consultation_channel = 'video' THEN EXISTS (
                       SELECT 1
                       FROM v2.appointment_facial_upload afu
                       WHERE afu.appointment_id = a.appointment_id
                   )
                   ELSE true
               END as is_facial_verified
        FROM v2.reservation r
                 LEFT JOIN v2.appointment a ON a.appointment_id = r.booking_id
                 LEFT JOIN v2.session_info si ON si.appointment_id = a.appointment_id
                 LEFT JOIN v2.appointment_payment_transaction pay ON pay.appointment_id = a.appointment_id
        WHERE r.booking_id = p_appointment_id
          AND (r.patient_profile_id = p_user_profile_id OR r.doctor_profile_id = p_user_profile_id)
          AND r.deleted_at IS NULL;
END;
$$;

-- ============================================
-- Upsert Payment Transaction
-- Creates appointment if booking_id doesn't exist (reservation should already exist)
-- Then upserts payment transaction
-- Returns appointment_id
-- ============================================
DROP FUNCTION IF EXISTS v2.upsert_payment_transaction CASCADE;
CREATE OR REPLACE FUNCTION v2.upsert_payment_transaction(
    p_booking_id varchar(20),
    p_payment_tx_ref_id varchar(255),
    p_payment_channels jsonb
) RETURNS varchar(20) AS $$
DECLARE
    v_appointment_id varchar(20);
    v_reservation RECORD;
    v_prescreen_id integer;
BEGIN
    SELECT * INTO v_reservation
    FROM v2.reservation r
    WHERE r.booking_id = p_booking_id AND r.deleted_at IS NULL;

    IF v_reservation IS NULL THEN
        RAISE EXCEPTION 'Reservation not found for booking_id: %', p_booking_id;
    END IF;

    SELECT a.appointment_id INTO v_appointment_id
    FROM v2.appointment a
    WHERE a.appointment_id = p_booking_id;

    SELECT prescreen_id INTO v_prescreen_id
    FROM v2.patient_prescreen
    WHERE booking_id = p_booking_id;

    IF v_appointment_id IS NULL THEN
        INSERT INTO v2.appointment (
            appointment_id,
            booking_id,
            prescreen_data_id,
            appointment_status,
            appointment_start,
            consult_duration,
            appointment_end,
            has_follow_up
        ) VALUES (
            p_booking_id,
            p_booking_id,
            COALESCE(v_prescreen_id, 0),
            'BOOKED'::v2.fhir_appointment_status_enum,
            v_reservation.appointment_start,
            v_reservation.appointment_end - v_reservation.appointment_start,
            v_reservation.appointment_end,
            false
        )
        RETURNING appointment_id INTO v_appointment_id;
    END IF;

    INSERT INTO v2.appointment_payment_transaction (appointment_id, payment_tx_ref_id, payment_channels)
    VALUES (p_booking_id, p_payment_tx_ref_id, p_payment_channels)
    ON CONFLICT (appointment_id)
    DO UPDATE SET
        payment_tx_ref_id = EXCLUDED.payment_tx_ref_id,
        payment_channels = EXCLUDED.payment_channels,
        modified_at = NOW();

    RETURN v_appointment_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================
-- Create Confirmed Appointment
-- Creates both a reservation and appointment in a single transaction
-- Returns booking_id and appointment details
-- ============================================
DROP FUNCTION IF EXISTS v2.create_confirmed_appointment CASCADE;

CREATE OR REPLACE FUNCTION v2.create_confirmed_appointment(
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
    p_appointment_start timestamptz,
    p_appointment_end timestamptz,
    p_prescreen_data text,
    p_prescreen_data_type varchar(255),
    p_parent_appointment_id varchar(20) DEFAULT NULL,
    p_payment_channels jsonb DEFAULT NULL
) RETURNS TABLE (
    booking_id varchar(20),
    appointment_id varchar(20),
    reservation_status v2.reservation_status_enum,
    appointment_status v2.fhir_appointment_status_enum,
    prescreen_data_id integer
) AS $$
DECLARE
    v_booking_id varchar(20);
    v_prescreen_data_id integer;
BEGIN
    v_booking_id := v2.generate_booking_id();

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
        v_booking_id,
        p_patient_account_id,
        p_patient_profile_id,
        p_doctor_id,
        p_doctor_account_id,
        p_doctor_profile_id,
        p_biz_unit_id,
        p_biz_center_id,
        p_tenant_id,
        'CONFIRMED'::v2.reservation_status_enum,
        p_appointment_end,
        p_booking_type,
        p_consultation_channel,
        p_appointment_start,
        p_appointment_end
    );

    INSERT INTO v2.patient_prescreen (
        booking_id,
        prescreen_data,
        prescreen_data_type,
        user_account_id,
        user_profile_id
    ) VALUES (
        v_booking_id,
        p_prescreen_data,
        p_prescreen_data_type,
        p_patient_account_id,
        p_patient_profile_id
    ) RETURNING prescreen_id INTO v_prescreen_data_id;

    INSERT INTO v2.appointment (
        appointment_id,
        booking_id,
        prescreen_data_id,
        parent_appointment_id,
        appointment_status,
        appointment_start,
        consult_duration,
        appointment_end,
        has_follow_up
    ) VALUES (
        v_booking_id,
        v_booking_id,
        v_prescreen_data_id,
        p_parent_appointment_id,
        'BOOKED'::v2.fhir_appointment_status_enum,
        p_appointment_start,
        p_appointment_end - p_appointment_start,
        p_appointment_end,
        false
    );

    INSERT INTO v2.appointment_payment_transaction (
        appointment_id,
        payment_tx_ref_id,
        payment_channels
    ) VALUES (
        v_booking_id,
        v2.generate_uuid_v7()::varchar,
        p_payment_channels
    );

    RETURN QUERY
    SELECT
        r.booking_id,
        a.appointment_id,
        r.reservation_status,
        a.appointment_status,
        a.prescreen_data_id
    FROM v2.reservation r
    INNER JOIN v2.appointment a ON a.appointment_id = r.booking_id
    WHERE r.booking_id = v_booking_id;
END;
$$ LANGUAGE plpgsql;
