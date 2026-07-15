-- sqlfluff:dialect:postgres

-- Payment confirmation must make the timeslot durable. Previously the
-- appointment could become BOOKED while reservation stayed RESERVED, allowing
-- the expiry worker to later emit ReservationExpired for an already-booked
-- consultation. This replacement confirms the reservation atomically with the
-- appointment/payment write and keeps the existing publication marker as an
-- idempotency gate for enqueuing ConsultationBooked into v2.event_outbox.
DROP FUNCTION IF EXISTS v2.upsert_payment_transaction(varchar, bigint, varchar, jsonb);
CREATE OR REPLACE FUNCTION v2.upsert_payment_transaction(
    p_booking_id varchar(20),
    p_payment_tx_id bigint,
    p_payment_tx_ref_id varchar(255),
    p_payment_channels jsonb
) RETURNS TABLE (
    booking_id varchar(20),
    patient_account_id integer,
    patient_profile_id integer,
    tenant_id integer,
    doctor_id integer,
    biz_unit_id integer,
    booking_type text,
    consultation_channel text,
    consultation_start_time bigint,
    consultation_duration_in_second integer,
    symptoms text,
    should_publish_consultation_booked boolean
) AS $$
DECLARE
    v_appointment_id varchar(20);
    v_current_status v2.fhir_appointment_status_enum;
    v_reservation RECORD;
    v_prescreen_id integer;
    v_state_changed boolean := false;
BEGIN
    SELECT * INTO v_reservation
    FROM v2.reservation r
    WHERE r.booking_id = p_booking_id AND r.deleted_at IS NULL
    FOR UPDATE;

    IF v_reservation IS NULL THEN
        RAISE EXCEPTION 'Reservation not found for booking_id: %', p_booking_id;
    END IF;

    IF v_reservation.reservation_status IN (
        'CANCELLED'::v2.reservation_status_enum,
        'RESERVE_EXPIRED'::v2.reservation_status_enum
    ) THEN
        RAISE EXCEPTION 'Cannot confirm payment for booking_id=%, reservation status is %',
            p_booking_id, v_reservation.reservation_status;
    END IF;

    IF v_reservation.reservation_status = 'RESERVED'::v2.reservation_status_enum
       AND v_reservation.reserved_until <= NOW() THEN
        UPDATE v2.reservation
        SET reservation_status = 'RESERVE_EXPIRED'::v2.reservation_status_enum,
            expired_at = COALESCE(expired_at, NOW()),
            cancelled_at = COALESCE(cancelled_at, NOW()),
            modified_at = NOW()
        WHERE reservation.booking_id = p_booking_id;

        RAISE EXCEPTION 'Cannot confirm payment for booking_id=%, reservation expired at %',
            p_booking_id, v_reservation.reserved_until;
    END IF;

    SELECT a.appointment_id, a.appointment_status
    INTO v_appointment_id, v_current_status
    FROM v2.appointment a
    WHERE a.appointment_id = p_booking_id
    FOR UPDATE;

    IF v_appointment_id IS NULL THEN
        SELECT prescreen_id INTO v_prescreen_id
        FROM v2.patient_prescreen
        WHERE patient_prescreen.booking_id = p_booking_id;

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
        RETURNING appointment.appointment_id INTO v_appointment_id;

        v_state_changed := true;
    ELSIF v_current_status = 'PENDING'::v2.fhir_appointment_status_enum THEN
        UPDATE v2.appointment
        SET appointment_status = 'BOOKED'::v2.fhir_appointment_status_enum,
            modified_at = NOW()
        WHERE appointment.appointment_id = p_booking_id;

        v_state_changed := true;
    ELSIF v_current_status = 'BOOKED'::v2.fhir_appointment_status_enum THEN
        RAISE NOTICE 'Idempotent payment confirm for booking_id=%, already BOOKED', p_booking_id;
    ELSE
        RAISE EXCEPTION 'Cannot confirm payment for booking_id=%, appointment status is %',
            p_booking_id, v_current_status;
    END IF;

    IF v_reservation.reservation_status <> 'CONFIRMED'::v2.reservation_status_enum THEN
        UPDATE v2.reservation
        SET reservation_status = 'CONFIRMED'::v2.reservation_status_enum,
            reserved_until = appointment_end,
            expiration_event_claimed_at = NULL,
            expiration_event_published_at = NULL,
            modified_at = NOW()
        WHERE reservation.booking_id = p_booking_id;

        v_state_changed := true;
    END IF;

    INSERT INTO v2.appointment_payment_transaction (
        appointment_id,
        payment_tx_id,
        payment_tx_ref_id,
        payment_channels
    )
    VALUES (p_booking_id, p_payment_tx_id, p_payment_tx_ref_id, p_payment_channels)
    ON CONFLICT (appointment_id)
    DO UPDATE SET
        payment_tx_id = EXCLUDED.payment_tx_id,
        payment_tx_ref_id = EXCLUDED.payment_tx_ref_id,
        payment_channels = EXCLUDED.payment_channels,
        modified_at = NOW();

    IF v_state_changed THEN
        INSERT INTO v2.appointment_event_publication (
            appointment_id,
            event_type,
            publication_status
        )
        VALUES (
            p_booking_id,
            'ConsultationBooked',
            'PENDING'
        )
        ON CONFLICT (appointment_id, event_type) DO NOTHING;
    END IF;

    RETURN QUERY
    SELECT
        r.booking_id,
        r.patient_account_id,
        r.patient_profile_id,
        r.tenant_id,
        r.doctor_id,
        COALESCE(r.biz_unit_id, 0) AS biz_unit_id,
        r.booking_type::text AS booking_type,
        r.consultation_channel::text AS consultation_channel,
        FLOOR(EXTRACT(EPOCH FROM r.appointment_start))::bigint AS consultation_start_time,
        FLOOR(EXTRACT(EPOCH FROM (r.appointment_end - r.appointment_start)))::integer
            AS consultation_duration_in_second,
        CASE
            WHEN ps.prescreen_data_type = 'RAW_JSON'
                THEN COALESCE((ps.prescreen_data::jsonb ->> 'symptom'), '')
            ELSE ''
        END AS symptoms,
        COALESCE(pub.publication_status = 'PENDING', false)
            AS should_publish_consultation_booked
    FROM v2.reservation r
    LEFT JOIN v2.patient_prescreen ps ON ps.booking_id = r.booking_id
    LEFT JOIN v2.appointment_event_publication pub
        ON pub.appointment_id = r.booking_id
        AND pub.event_type = 'ConsultationBooked'
    WHERE r.booking_id = p_booking_id;
END;
$$ LANGUAGE plpgsql;
