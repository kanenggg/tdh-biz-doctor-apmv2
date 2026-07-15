-- sqlfluff:dialect:postgres

-- ============================================
-- Payment Confirm: Idempotent upsert_payment_transaction
-- ============================================
-- Updates v2.upsert_payment_transaction to:
-- 1. Accept payment_tx_id (bigint)
-- 2. Handle idempotent appointment status transitions (PENDING -> BOOKED)
-- 3. Log idempotent replays via RAISE NOTICE

DROP FUNCTION IF EXISTS v2.upsert_payment_transaction CASCADE;
CREATE OR REPLACE FUNCTION v2.upsert_payment_transaction(
    p_booking_id varchar(20),
    p_payment_tx_id bigint,
    p_payment_tx_ref_id varchar(255),
    p_payment_channels jsonb
) RETURNS varchar(20) AS $$
DECLARE
    v_appointment_id varchar(20);
    v_current_status v2.fhir_appointment_status_enum;
    v_reservation RECORD;
    v_prescreen_id integer;
BEGIN
    -- Get reservation (must exist)
    SELECT * INTO v_reservation
    FROM v2.reservation r
    WHERE r.booking_id = p_booking_id AND r.deleted_at IS NULL;

    IF v_reservation IS NULL THEN
        RAISE EXCEPTION 'Reservation not found for booking_id: %', p_booking_id;
    END IF;

    -- Check if appointment exists
    SELECT a.appointment_id, a.appointment_status
    INTO v_appointment_id, v_current_status
    FROM v2.appointment a
    WHERE a.appointment_id = p_booking_id;

    IF v_appointment_id IS NULL THEN
        -- Get prescreen_data_id from patient_prescreen
        SELECT prescreen_id INTO v_prescreen_id
        FROM v2.patient_prescreen
        WHERE booking_id = p_booking_id;

        -- Create appointment with BOOKED status
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
    ELSIF v_current_status = 'PENDING'::v2.fhir_appointment_status_enum THEN
        -- Transition PENDING -> BOOKED
        UPDATE v2.appointment
        SET appointment_status = 'BOOKED'::v2.fhir_appointment_status_enum,
            modified_at = NOW()
        WHERE appointment_id = p_booking_id;
    ELSIF v_current_status = 'BOOKED'::v2.fhir_appointment_status_enum THEN
        -- Idempotent replay — skip status update
        RAISE NOTICE 'Idempotent payment confirm for booking_id=%, already BOOKED', p_booking_id;
    ELSE
        RAISE EXCEPTION 'Cannot confirm payment for booking_id=%, appointment status is %', p_booking_id, v_current_status;
    END IF;

    -- Upsert payment transaction
    INSERT INTO v2.appointment_payment_transaction (appointment_id, payment_tx_id, payment_tx_ref_id, payment_channels)
    VALUES (p_booking_id, p_payment_tx_id, p_payment_tx_ref_id, p_payment_channels)
    ON CONFLICT (appointment_id)
    DO UPDATE SET
        payment_tx_id = EXCLUDED.payment_tx_id,
        payment_tx_ref_id = EXCLUDED.payment_tx_ref_id,
        payment_channels = EXCLUDED.payment_channels,
        modified_at = NOW();

    RETURN v_appointment_id;
END;
$$ LANGUAGE plpgsql;
