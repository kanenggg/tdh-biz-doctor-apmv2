-- sqlfluff:dialect:postgres

-- ============================================
-- Payment Confirm: ConsultationBooked publication marker
-- ============================================
-- Records whether the ConsultationBooked event derived from payment confirm has
-- been published. This lets Pub/Sub redelivery retry a failed publish while
-- skipping already-published replays.

CREATE TABLE IF NOT EXISTS v2.appointment_event_publication (
    appointment_id varchar(20) NOT NULL,
    event_type varchar(255) NOT NULL,
    publication_status varchar(32) NOT NULL DEFAULT 'PENDING',
    published_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    modified_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (appointment_id, event_type)
);

CREATE TRIGGER update_appointment_event_publication_modified_at
BEFORE UPDATE ON v2.appointment_event_publication
FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();

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
    WHERE r.booking_id = p_booking_id AND r.deleted_at IS NULL;

    IF v_reservation IS NULL THEN
        RAISE EXCEPTION 'Reservation not found for booking_id: %', p_booking_id;
    END IF;

    SELECT a.appointment_id, a.appointment_status
    INTO v_appointment_id, v_current_status
    FROM v2.appointment a
    WHERE a.appointment_id = p_booking_id;

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
        RAISE EXCEPTION 'Cannot confirm payment for booking_id=%, appointment status is %', p_booking_id, v_current_status;
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
