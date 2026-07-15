-- sqlfluff:dialect:postgres

-- ============================================
-- UUID v7 Generator
-- ============================================
DROP FUNCTION IF EXISTS v2.generate_uuid_v7() CASCADE;
CREATE OR REPLACE FUNCTION v2.generate_uuid_v7() RETURNS uuid AS $$
DECLARE
    timestamp_ms bigint;
    uuid_hex text;
BEGIN
    -- Get current timestamp in milliseconds
    timestamp_ms := (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::bigint;
    -- Construct hex string: 12 hex chars for timestamp + random bits
    uuid_hex := lpad(to_hex(timestamp_ms), 12, '0') || '7' || substr(to_hex((random() * 15)::int), 1, 1) || substr(md5(random()::text), 1, 18);

    RETURN uuid_hex::uuid;
END;
$$ LANGUAGE plpgsql VOLATILE;

-- ============================================
-- Booking ID Generator
-- Creates a unique booking ID by combining date prefix (YYYYMMDD) with running number from sequence
-- Returns a varchar(20) booking_id
-- ============================================
DROP FUNCTION IF EXISTS v2.generate_booking_id() CASCADE;
CREATE OR REPLACE FUNCTION v2.generate_booking_id() RETURNS varchar(20) AS $$
DECLARE
    v_running_number integer;
    v_date_prefix text;
    v_booking_id varchar(20);
BEGIN
    -- Get next running number from sequence
    v_running_number := nextval('v2.reservation_booking_id_seq');

    -- Generate date prefix in YYYYMMDD format
    v_date_prefix := to_char(CURRENT_DATE, 'YYYYMMDD');

    -- Combine date prefix with running number to create booking_id
    v_booking_id := v_date_prefix || v_running_number::text;

    RETURN v_booking_id;
END;
$$ LANGUAGE plpgsql VOLATILE;

-- ============================================
-- Create Reservation
-- Creates a new reservation in reservation table
-- Returns reservation_id and reservation details
-- ============================================
DROP FUNCTION IF EXISTS v2.create_reservation CASCADE;

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
    p_ttl_seconds integer
) RETURNS TABLE (
    booking_id varchar(20)
) AS $$
DECLARE
    v_reservation_id varchar(20);
BEGIN
    -- Generate unique booking_id
    v_reservation_id := v2.generate_booking_id();

    -- Insert into reservation
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
        p_reserved_from + (p_ttl_seconds || ' seconds')::interval,
        p_booking_type,
        p_consultation_channel,
        p_reserved_from,
        p_reserved_from + (p_ttl_seconds || ' seconds')::interval
    );

    -- Return created reservation booking_id
    RETURN QUERY
    SELECT
        r.booking_id
    FROM v2.reservation r
    WHERE r.booking_id = v_reservation_id;
END;
$$ LANGUAGE plpgsql;

-- ============================================
-- Get Reservation By ID
-- Retrieves a reservation by its ID
-- Returns NULL if reservation doesn't exist or is deleted
-- ============================================
DROP FUNCTION IF EXISTS v2.get_reservation_by_id CASCADE;
CREATE OR REPLACE FUNCTION v2.get_reservation_by_id(
    p_reservation_id varchar(20)
) RETURNS TABLE (
    booking_id varchar(20),
    patient_account_id integer,
    patient_profile_id integer,
    doctor_id integer,
    doctor_account_id integer,
    doctor_profile_id integer,
    biz_unit_id integer,
    biz_center_id integer,
    tenant_id integer,
    reservation_status v2.reservation_status_enum,
    reserved_until timestamptz,
    booking_type v2.booking_type_enum,
    consultation_channel v2.consultation_type_enum,
    appointment_start timestamptz,
    appointment_end timestamptz
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        r.booking_id,
        r.patient_account_id,
        r.patient_profile_id,
        r.doctor_id,
        r.doctor_account_id,
        r.doctor_profile_id,
        r.biz_unit_id,
        r.biz_center_id,
        r.tenant_id,
        r.reservation_status,
        r.reserved_until,
        r.booking_type,
        r.consultation_channel,
        r.appointment_start,
        r.appointment_end
    FROM v2.reservation r
    WHERE r.booking_id = p_reservation_id
      AND r.deleted_at IS NULL;
END;
$$ LANGUAGE plpgsql STABLE;

-- ============================================
-- Get Doctor Reservations For Date Range
-- Returns all non-deleted reservations for a doctor within a date range
-- ============================================
DROP FUNCTION IF EXISTS v2.get_doctor_reservations CASCADE;
CREATE OR REPLACE FUNCTION v2.get_doctor_reservations(
    p_doctor_id integer,
    p_start_date date,
    p_end_date date
) RETURNS TABLE (
    booking_id varchar(20),
    appointment_start timestamptz,
    appointment_end timestamptz
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        r.booking_id,
        r.appointment_start,
        r.appointment_end
    FROM v2.reservation r
    WHERE r.doctor_id = p_doctor_id
      AND r.deleted_at IS NULL
      AND r.appointment_start::date >= p_start_date
      AND r.appointment_start::date <= p_end_date;
END;
$$ LANGUAGE plpgsql STABLE;

-- ============================================
-- Generate Timeslots For Date
-- Generates timeslots based on working hours, slot duration, and gap
-- ============================================
DROP FUNCTION IF EXISTS v2.generate_timeslots_for_date CASCADE;
CREATE OR REPLACE FUNCTION v2.generate_timeslots_for_date(
    p_date date,
    p_working_start_time time,
    p_working_end_time time,
    p_slot_duration_minutes integer,
    p_gap_minutes integer
) RETURNS TABLE (
    slot_date date,
    slot_start_time time,
    slot_end_time time
) AS $$
DECLARE
    v_current_time time := p_working_start_time;
    v_slot_end_time time;
BEGIN
    -- Generate timeslots until end of working hours
    WHILE v_current_time + (p_slot_duration_minutes || ' minutes')::interval <= p_working_end_time LOOP
        v_slot_end_time := v_current_time + (p_slot_duration_minutes || ' minutes')::interval;

        RETURN QUERY SELECT
            p_date,
            v_current_time,
            v_slot_end_time;

        v_current_time := v_slot_end_time + (p_gap_minutes || ' minutes')::interval;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

-- ============================================
-- Get Available Timeslots
-- Returns available timeslots by checking against existing reservations
-- ============================================
DROP FUNCTION IF EXISTS v2.get_available_timeslots CASCADE;
CREATE OR REPLACE FUNCTION v2.get_available_timeslots(
    p_doctor_id integer,
    p_start_date date,
    p_end_date date,
    p_working_start_time time DEFAULT '09:00',
    p_working_end_time time DEFAULT '17:00',
    p_slot_duration_minutes integer DEFAULT 30,
    p_gap_minutes integer DEFAULT 5
) RETURNS TABLE (
    slot_id bigint,
    slot_date date,
    slot_start_time time,
    slot_end_time time,
    slot_start_timestamptz timestamptz,
    slot_end_timestamptz timestamptz
) AS $$
DECLARE
    v_current_date date := p_start_date;
    v_slot_counter bigint := 1;
    v_working_start_time time;
    v_working_end_time time;
    v_date_time timestamptz;
    v_reservation RECORD;
    v_timeslot RECORD;
BEGIN
    -- Iterate through each date in range
    WHILE v_current_date <= p_end_date LOOP
        v_date_time := v_current_date::timestamp with time zone;

        -- Generate timeslots for this date
        FOR v_timeslot IN
            SELECT
                ts.slot_date,
                ts.slot_start_time,
                ts.slot_end_time
            FROM v2.generate_timeslots_for_date(
                v_current_date,
                p_working_start_time,
                p_working_end_time,
                p_slot_duration_minutes,
                p_gap_minutes
            ) ts
        LOOP
            -- Check if this timeslot conflicts with any reservation
            IF NOT EXISTS (
                SELECT 1
                FROM v2.reservation r
                WHERE r.doctor_id = p_doctor_id
                  AND r.deleted_at IS NULL
                  AND r.reservation_status IN ('RESERVED', 'CONFIRMED', 'SCHEDULED')
                  AND r.appointment_start::date = v_current_date
                  AND (
                      (r.appointment_start <= v_timeslot.slot_date::timestamptz + v_timeslot.slot_start_time)
                      AND
                      (r.appointment_end >= v_timeslot.slot_date::timestamptz + v_timeslot.slot_end_time)
                  )
            ) THEN
                -- Timeslot is available, return it
                RETURN QUERY SELECT
                    v_slot_counter,
                    v_timeslot.slot_date,
                    v_timeslot.slot_start_time,
                    v_timeslot.slot_end_time,
                    v_timeslot.slot_date::timestamptz + v_timeslot.slot_start_time,
                    v_timeslot.slot_date::timestamptz + v_timeslot.slot_end_time;

                v_slot_counter := v_slot_counter + 1;
            END IF;
        END LOOP;

        v_current_date := v_current_date + 1;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

-- ============================================
-- Get Consultation Session
-- Joins v2.reservation, v2.appointment, and v2.session_info tables to return unified session info
-- Note: appointment_id = booking_id (1:1 relationship)
-- is_facial_verified: true if video channel and facial upload exists, false if video channel and no upload, true for non-video channels
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
    appointment_status v2.appointment_status_enum,
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
-- Upsert Session Info
-- Creates or updates session_info record with session data from Twilio
-- Note: appointment_id = booking_id (1:1 relationship)
-- ============================================
DROP FUNCTION IF EXISTS v2.upsert_session_info CASCADE;
CREATE OR REPLACE FUNCTION v2.upsert_session_info(
    p_appointment_id varchar(20),
    p_session_data jsonb
) RETURNS void AS $$
BEGIN
    -- Insert or update session_info
    INSERT INTO v2.session_info (
        appointment_id,
        session_provider,
        session_data
    )
    VALUES (
        p_appointment_id,
        'TWILIO'::text,
        p_session_data
    )
    ON CONFLICT (appointment_id)
    DO UPDATE SET
        session_data = EXCLUDED.session_data,
        modified_at = NOW();
END;
$$ LANGUAGE plpgsql;

-- ============================================
-- End Active Session
-- Updates appointment status to CONSULTATION_DONE and returns rows affected
-- ============================================
DROP FUNCTION IF EXISTS v2.end_active_session CASCADE;
create function end_active_session(
    p_appointment_id varchar(20),
    p_doctor_profile_id bigint
) returns bigint
language plpgsql
as
$$
DECLARE
    v_rows_affected bigint;
BEGIN
    -- Update appointment status to CONSULTATION_DONE
    UPDATE v2.appointment a
    SET appointment_status = 'CONSULTATION_DONE'::v2.appointment_status_enum,
        modified_at = NOW()
    WHERE a.appointment_id = p_appointment_id
      AND EXISTS (
          SELECT 1
          FROM v2.reservation r
          WHERE r.booking_id = a.booking_id
            AND r.doctor_profile_id = p_doctor_profile_id
      );

    -- Get number of rows affected
    GET DIAGNOSTICS v_rows_affected = ROW_COUNT;

    RETURN v_rows_affected;
END;
$$;

-- ============================================
-- Get Session Details
-- Retrieves session details including provider and chat ID for ending a session
-- ============================================
DROP FUNCTION IF EXISTS v2.get_session_details CASCADE;
CREATE OR REPLACE FUNCTION v2.get_session_details(
    p_appointment_id varchar(20)
) RETURNS TABLE (
    appointment_id varchar(20),
    booking_id varchar(20),
    patient_account_id integer,
    patient_profile_id integer,
    tenant_id integer,
    doctor_id integer,
    doctor_profile_id integer,
    session_provider text,
    session_chat_id text
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        a.appointment_id,
        r.booking_id,
        r.patient_account_id,
        r.patient_profile_id,
        r.tenant_id,
        r.doctor_id,
        r.doctor_profile_id,
        COALESCE(si.session_provider, 'TWILIO')::text,
        si.session_data->>'sessionChatId'
    FROM v2.appointment a
    INNER JOIN v2.reservation r ON r.booking_id = a.booking_id
    LEFT JOIN v2.session_info si ON si.appointment_id = a.appointment_id
    WHERE a.appointment_id = p_appointment_id;
END;
$$ LANGUAGE plpgsql STABLE;

-- ============================================
-- Add Patient Verification Transaction
-- Inserts a record in patient_id_verification_transaction table to mark patient ID verification
-- Returns 1 if appointment exists and record was inserted or already exists, 0 otherwise
-- ============================================
DROP FUNCTION IF EXISTS v2.add_patient_verification_tx CASCADE;
CREATE OR REPLACE FUNCTION v2.add_patient_verification_tx(
    p_appointment_id varchar(20),
    p_doctor_profile_id bigint
) RETURNS bigint AS $$
DECLARE
    v_appointment_exists bigint;
    v_rows_affected bigint;
BEGIN
    -- Check if appointment exists and belongs to specified doctor
    SELECT COUNT(*)
    INTO v_appointment_exists
    FROM v2.reservation r
    INNER JOIN v2.appointment a ON a.appointment_id = r.booking_id
    WHERE a.appointment_id = p_appointment_id
      AND r.doctor_profile_id = p_doctor_profile_id
      AND r.deleted_at IS NULL;

    -- If appointment doesn't exist or doesn't belong to doctor, return 0
    IF v_appointment_exists = 0 THEN
        RETURN 0;
    END IF;

    -- Insert into patient_id_verification_transaction
    -- On conflict (appointment_id already exists), do nothing
    INSERT INTO v2.patient_id_verification_transaction (appointment_id)
    VALUES (p_appointment_id)
    ON CONFLICT (appointment_id)
    DO NOTHING;

    -- Get number of rows affected
    GET DIAGNOSTICS v_rows_affected = ROW_COUNT;

    -- Return 1 if inserted or already exists (appointment exists and belongs to doctor)
    RETURN 1;
END;
$$ LANGUAGE plpgsql;

-- ============================================
-- Add Patient ID Verification (alias for add_patient_verification_tx)
-- This is the function name used by the patient verification service
-- ============================================
DROP FUNCTION IF EXISTS v2.add_patient_id_verification CASCADE;
CREATE OR REPLACE FUNCTION v2.add_patient_id_verification(
    p_booking_id varchar(20),
    p_doctor_id bigint
) RETURNS bigint AS $$
BEGIN
    RETURN v2.add_patient_verification_tx(p_booking_id, p_doctor_id);
END;
$$ LANGUAGE plpgsql;

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
    -- Update appointment status to CANCELLED
    UPDATE v2.appointment a
    SET appointment_status = 'CANCELLED'::v2.appointment_status_enum,
        modified_at = NOW()
    WHERE a.appointment_id = p_booking_id
      AND EXISTS (
          SELECT 1
          FROM v2.reservation r
          WHERE r.booking_id = a.appointment_id
            AND r.doctor_profile_id = p_doctor_id
            AND r.deleted_at IS NULL
      );

    -- Get number of rows affected
    GET DIAGNOSTICS v_rows_affected = ROW_COUNT;

    RETURN v_rows_affected;
END;
$$ LANGUAGE plpgsql;

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
    -- Get reservation (must exist)
    SELECT * INTO v_reservation
    FROM v2.reservation r
    WHERE r.booking_id = p_booking_id AND r.deleted_at IS NULL;

    IF v_reservation IS NULL THEN
        RAISE EXCEPTION 'Reservation not found for booking_id: %', p_booking_id;
    END IF;

    -- Check if appointment exists
    SELECT a.appointment_id INTO v_appointment_id
    FROM v2.appointment a
    WHERE a.appointment_id = p_booking_id;

    -- Get prescreen_data_id from patient_prescreen
    SELECT prescreen_id INTO v_prescreen_id
    FROM v2.patient_prescreen
    WHERE booking_id = p_booking_id;

    -- If appointment doesn't exist, create it
    IF v_appointment_id IS NULL THEN
        -- Create appointment
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
            'CONFIRMED'::v2.appointment_status_enum,
            v_reservation.appointment_start,
            v_reservation.appointment_end - v_reservation.appointment_start,
            v_reservation.appointment_end,
            false
        )
        RETURNING appointment_id INTO v_appointment_id;
    END IF;

    -- Upsert payment transaction
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
