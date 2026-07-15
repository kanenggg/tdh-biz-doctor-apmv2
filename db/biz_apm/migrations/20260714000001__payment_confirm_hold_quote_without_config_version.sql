-- Instant Meeting Payment Summary V2 does not transport consultationConfigVersion.
-- The Appointment Hold remains the source of the immutable quote and retains its
-- configuration version only as audit provenance.
DROP FUNCTION IF EXISTS v2.confirm_payment_and_enqueue_consultation_booked(
    varchar, bigint, varchar, jsonb, numeric, varchar, bigint, integer, bigint, varchar
);
DROP FUNCTION IF EXISTS v2.confirm_payment_and_enqueue_consultation_booked(
    varchar, bigint, varchar, jsonb, numeric, varchar, integer, bigint, varchar
);

CREATE FUNCTION v2.confirm_payment_and_enqueue_consultation_booked(
    p_booking_id varchar(20), p_payment_tx_id bigint, p_payment_tx_ref_id varchar(255),
    p_payment_channels jsonb, p_payment_amount numeric, p_payment_currency varchar(16),
    p_payment_module_id integer, p_booked_at bigint, p_consultation_topic varchar(255)
) RETURNS void AS $$
DECLARE
    v_appointment_id varchar(20); v_hold v2.appointment_hold%ROWTYPE;
    v_existing_payment v2.appointment_payment_transaction%ROWTYPE;
    v_prescreen_id integer; v_active_occupancy_count integer; v_event_id uuid;
BEGIN
    SELECT appointment_id INTO v_appointment_id
    FROM v2.appointment WHERE booking_id = p_booking_id FOR UPDATE;

    SELECT * INTO v_hold FROM v2.appointment_hold
    WHERE booking_id = p_booking_id FOR UPDATE;
    IF NOT FOUND THEN
        RAISE EXCEPTION 'appointment hold not found' USING ERRCODE = 'P0002';
    END IF;

    IF v_appointment_id IS NOT NULL THEN
        SELECT * INTO v_existing_payment FROM v2.appointment_payment_transaction
        WHERE appointment_id = v_appointment_id FOR UPDATE;
        IF NOT FOUND
           OR v_existing_payment.payment_tx_id IS DISTINCT FROM p_payment_tx_id
           OR v_existing_payment.payment_tx_ref_id IS DISTINCT FROM p_payment_tx_ref_id
           OR v_existing_payment.payment_channels IS DISTINCT FROM p_payment_channels
           OR v_existing_payment.payment_amount IS DISTINCT FROM p_payment_amount
           OR v_existing_payment.payment_currency IS DISTINCT FROM p_payment_currency
           OR v_existing_payment.consultation_config_version IS DISTINCT FROM v_hold.quoted_service_config_version
           OR v_existing_payment.payment_module_id IS DISTINCT FROM p_payment_module_id
           OR v_existing_payment.booked_at IS DISTINCT FROM p_booked_at THEN
            RAISE EXCEPTION 'conflicting payment replay' USING ERRCODE = 'P1002';
        END IF;
    ELSE
        IF v_hold.hold_status <> 'ACTIVE'::v2.appointment_hold_status_enum THEN
            RAISE EXCEPTION 'hold is no longer bookable' USING ERRCODE = 'P1001';
        END IF;
        IF v_hold.expires_at <= NOW() THEN
            RAISE EXCEPTION 'hold is due for expiry' USING ERRCODE = 'P1004';
        END IF;
        IF v_hold.quoted_amount IS NULL OR v_hold.quoted_currency IS NULL
           OR p_payment_amount <> v_hold.quoted_amount
           OR p_payment_currency <> upper(btrim(p_payment_currency))
           OR p_payment_currency <> v_hold.quoted_currency THEN
            RAISE EXCEPTION 'payment does not match immutable hold quote' USING ERRCODE = 'P1005';
        END IF;
        SELECT count(*) INTO v_active_occupancy_count FROM (
            SELECT occupancy_id FROM v2.doctor_occupancy
            WHERE hold_id = v_hold.hold_id
              AND occupancy_status = 'ACTIVE'::v2.doctor_occupancy_status_enum
            FOR UPDATE
        ) AS locked_occupancy;
        IF v_active_occupancy_count <> 1 THEN
            RAISE EXCEPTION 'active hold occupancy missing or duplicated' USING ERRCODE = 'P1003';
        END IF;
        v_prescreen_id := v_hold.source_prescreen_id;
        IF v_prescreen_id IS NULL THEN
            RAISE EXCEPTION 'active hold must own a prescreen' USING ERRCODE = '22023';
        END IF;
        v_appointment_id := v2.generate_appointment_id();
        IF v_appointment_id = p_booking_id THEN
            RAISE EXCEPTION 'generated appointment identity conflicts with booking identity' USING ERRCODE = 'P1003';
        END IF;
        INSERT INTO v2.appointment (
            appointment_id, booking_id, prescreen_data_id, appointment_status,
            appointment_start, consult_duration, appointment_end, has_follow_up,
            source_hold_id, source_hold_prescreen_id, patient_account_id,
            patient_profile_id, doctor_id, doctor_account_id, doctor_profile_id,
            biz_unit_id, biz_center_id, tenant_id, booking_type, consultation_channel
        ) VALUES (
            v_appointment_id, p_booking_id, v_prescreen_id,
            'BOOKED'::v2.fhir_appointment_status_enum, v_hold.starts_at,
            v_hold.ends_at - v_hold.starts_at, v_hold.ends_at, false, v_hold.hold_id,
            v_prescreen_id, v_hold.patient_account_id, v_hold.patient_profile_id,
            v_hold.doctor_id, v_hold.doctor_account_id, v_hold.doctor_profile_id,
            v_hold.biz_unit_id, v_hold.biz_center_id, v_hold.tenant_id,
            v_hold.booking_type, v_hold.consultation_channel
        );
        UPDATE v2.doctor_occupancy SET hold_id = NULL, appointment_id = v_appointment_id,
            modified_at = NOW()
        WHERE hold_id = v_hold.hold_id
          AND occupancy_status = 'ACTIVE'::v2.doctor_occupancy_status_enum;
        IF NOT FOUND THEN
            RAISE EXCEPTION 'active hold occupancy disappeared during transfer' USING ERRCODE = 'P1003';
        END IF;
        UPDATE v2.appointment_hold SET hold_status = 'RELEASED', released_at = NOW(),
            release_reason = 'Booked', confirmed_appointment_id = v_appointment_id,
            modified_at = NOW() WHERE hold_id = v_hold.hold_id;
        INSERT INTO v2.appointment_payment_transaction (
            appointment_id, payment_tx_id, payment_tx_ref_id, payment_channels,
            payment_amount, payment_currency, consultation_config_version,
            payment_module_id, booked_at
        ) VALUES (
            v_appointment_id, p_payment_tx_id, p_payment_tx_ref_id, p_payment_channels,
            p_payment_amount, p_payment_currency, v_hold.quoted_service_config_version,
            p_payment_module_id, p_booked_at
        );
    END IF;

    v_event_id := v2.generate_uuid_v7();
    INSERT INTO v2.event_outbox (
        event_id, topic, event_type, aggregate_id, payload, publication_status
    ) VALUES (
        v_event_id, p_consultation_topic, 'ConsultationBooked', p_booking_id,
        jsonb_build_object(
            '__type', 'ConsultationBooked', 'eventId', v_event_id::text,
            'bookingId', p_booking_id,
            'patientIdentity', jsonb_build_object('accountId', v_hold.patient_account_id,
                'userProfileId', v_hold.patient_profile_id, 'tenantId', v_hold.tenant_id),
            'doctorId', v_hold.doctor_id, 'bizUnitId', COALESCE(v_hold.biz_unit_id, 0),
            'paymentModuleId', p_payment_module_id, 'bookingType', v_hold.booking_type::text,
            'consultationStartTime', FLOOR(EXTRACT(EPOCH FROM v_hold.starts_at))::bigint,
            'consultationDurationInSecond', FLOOR(EXTRACT(EPOCH FROM v_hold.ends_at - v_hold.starts_at))::integer,
            'consultationChannel', v_hold.consultation_channel::text,
            'bookedAt', p_booked_at, 'symptoms', COALESCE((v_hold.prescreen_payload ->> 'symptom'), ''),
            'consultationFee', v_hold.quoted_amount
        ), 'PENDING'
    ) ON CONFLICT DO NOTHING;
END;
$$ LANGUAGE plpgsql;
