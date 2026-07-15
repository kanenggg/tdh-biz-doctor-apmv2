CREATE OR REPLACE FUNCTION v2.mark_appointment_has_follow_up(
    p_appointment_id varchar(20)
) RETURNS boolean AS $$
DECLARE
    v_current_status v2.fhir_appointment_status_enum;
BEGIN
    SELECT appointment_status INTO v_current_status
    FROM v2.appointment
    WHERE appointment_id = p_appointment_id;

    IF v_current_status IS NULL THEN
        RAISE EXCEPTION 'Appointment not found: %', p_appointment_id;
    END IF;

    IF v_current_status != 'FULFILLED' THEN
        RETURN false;
    END IF;

    UPDATE v2.appointment
    SET has_follow_up = true, modified_at = NOW()
    WHERE appointment_id = p_appointment_id
      AND appointment_status = 'FULFILLED';

    RETURN true;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION v2.create_follow_up_appointment(
    p_parent_appointment_id varchar(20),
    p_appointment_start timestamptz,
    p_consult_duration interval,
    p_appointment_type v2.appointment_type_enum DEFAULT 'ROUTINE'
) RETURNS jsonb AS $$
DECLARE
    v_parent RECORD;
    v_booking_id varchar(20);
    v_appointment_end timestamptz;
    v_prescreen_id integer;
BEGIN
    SELECT * INTO v_parent
    FROM v2.reservation r
    JOIN v2.appointment a ON a.appointment_id = r.booking_id
    WHERE a.appointment_id = p_parent_appointment_id
      AND r.deleted_at IS NULL;

    IF v_parent IS NULL THEN
        RAISE EXCEPTION 'Parent appointment not found: %', p_parent_appointment_id;
    END IF;

    IF v_parent.appointment_status != 'FULFILLED' THEN
        RAISE EXCEPTION 'Parent appointment is not FULFILLED: %', p_parent_appointment_id;
    END IF;

    v_booking_id := v2.generate_booking_id();
    v_appointment_end := p_appointment_start + p_consult_duration;

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
        v_parent.patient_account_id,
        v_parent.patient_profile_id,
        v_parent.doctor_id,
        v_parent.doctor_account_id,
        v_parent.doctor_profile_id,
        v_parent.biz_unit_id,
        v_parent.biz_center_id,
        v_parent.tenant_id,
        'CONFIRMED'::v2.reservation_status_enum,
        v_appointment_end,
        'FollowUp'::v2.booking_type_enum,
        v_parent.consultation_channel,
        p_appointment_start,
        v_appointment_end
    );

    INSERT INTO v2.patient_prescreen (
        booking_id,
        prescreen_data,
        prescreen_data_type,
        user_account_id,
        user_profile_id
    ) VALUES (
        v_booking_id,
        '{}',
        'FOLLOW_UP',
        v_parent.patient_account_id,
        v_parent.patient_profile_id
    ) RETURNING prescreen_id INTO v_prescreen_id;

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
        COALESCE(v_prescreen_id, 0),
        p_parent_appointment_id,
        'BOOKED'::v2.fhir_appointment_status_enum,
        p_appointment_start,
        p_consult_duration,
        v_appointment_end,
        false
    );

    UPDATE v2.appointment
    SET has_follow_up = true, modified_at = NOW()
    WHERE appointment_id = p_parent_appointment_id;

    RETURN jsonb_build_object(
        'bookingId', v_booking_id,
        'appointmentId', v_booking_id,
        'appointmentStart', EXTRACT(EPOCH FROM p_appointment_start)::bigint,
        'appointmentEnd', EXTRACT(EPOCH FROM v_appointment_end)::bigint,
        'doctorId', v_parent.doctor_id,
        'doctorProfileId', v_parent.doctor_profile_id,
        'consultationChannel', v_parent.consultation_channel::text,
        'bizUnitId', v_parent.biz_unit_id,
        'bizCenterId', v_parent.biz_center_id,
        'tenantId', v_parent.tenant_id
    );
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION v2.get_appointment_chain(
    p_appointment_id varchar(20)
) RETURNS jsonb AS $$
DECLARE
    v_chain jsonb;
BEGIN
    WITH RECURSIVE chain AS (
        SELECT a.*, r.patient_account_id, r.patient_profile_id,
               r.doctor_id, r.doctor_account_id, r.doctor_profile_id,
               r.biz_unit_id, r.biz_center_id
        FROM v2.appointment a
        JOIN v2.reservation r ON r.booking_id = a.booking_id
        WHERE a.appointment_id = p_appointment_id
          AND r.deleted_at IS NULL

        UNION ALL

        SELECT child.*, r.patient_account_id, r.patient_profile_id,
               r.doctor_id, r.doctor_account_id, r.doctor_profile_id,
               r.biz_unit_id, r.biz_center_id
        FROM v2.appointment child
        JOIN v2.reservation r ON r.booking_id = child.booking_id
        JOIN chain c ON child.parent_appointment_id = c.appointment_id
        WHERE r.deleted_at IS NULL
    )
    SELECT COALESCE(jsonb_agg(
        jsonb_build_object(
            'appointmentId', chain.appointment_id,
            'parentAppointmentId', chain.parent_appointment_id,
            'appointmentStatus', chain.appointment_status::text,
            'appointmentStart', EXTRACT(EPOCH FROM chain.appointment_start)::bigint,
            'appointmentEnd', EXTRACT(EPOCH FROM chain.appointment_end)::bigint,
            'hasFollowUp', chain.has_follow_up,
            'patientAccountId', chain.patient_account_id,
            'doctorId', chain.doctor_id,
            'bizUnitId', chain.biz_unit_id,
            'bizCenterId', chain.biz_center_id
        )
        ORDER BY chain.appointment_start
    ), '[]'::jsonb) INTO v_chain
    FROM chain;

    RETURN v_chain;
END;
$$ LANGUAGE plpgsql STABLE;
