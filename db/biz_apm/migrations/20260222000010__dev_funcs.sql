-- sqlfluff:dialect:postgres

-- Create confirmed appointment with reservation
-- Creates both a reservation and appointment in a single transaction
-- Returns booking_id and appointment details
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
    appointment_status v2.appointment_status_enum,
    prescreen_data_id integer
) AS $$
DECLARE
    v_booking_id varchar(20);
    v_prescreen_data_id integer;
BEGIN
    -- Generate unique booking_id
    v_booking_id := v2.generate_booking_id();

    -- Insert into reservation with CONFIRMED status
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

    -- Create prescreen_data if provided
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

    -- Insert into appointment with CONFIRMED status
    -- Note: appointment_id = booking_id (1:1 relationship)
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
        'CONFIRMED'::v2.appointment_status_enum,
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

    -- Return the created booking and appointment details
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
