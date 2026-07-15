-- =============================================================================
-- v2 schema baseline
-- Consolidated squash of all incremental migrations as of 2026-06-22.
-- Generated via pg_dump --schema-only against a DB with every migration applied.
-- Data-migration / backfill steps are intentionally excluded (state-only).
-- Sections below (pg_dump order): SCHEMA -> TYPES -> FUNCTIONS -> TABLES
--   -> SEQUENCES/DEFAULTS -> CONSTRAINTS -> INDEXES -> TRIGGERS.
-- =============================================================================

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

CREATE SCHEMA IF NOT EXISTS v2;


--
-- Name: appointment_status_enum; Type: TYPE; Schema: v2; Owner: -
--

CREATE TYPE v2.appointment_status_enum AS ENUM (
    'PENDING',
    'CONFIRMED',
    'CONSULTATION_DONE',
    'CANCELLED'
);


--
-- Name: appointment_type_enum; Type: TYPE; Schema: v2; Owner: -
--

CREATE TYPE v2.appointment_type_enum AS ENUM (
    'ROUTINE',
    'WALK_IN',
    'EMERGENCY',
    'URGENT'
);


--
-- Name: booking_type_enum; Type: TYPE; Schema: v2; Owner: -
--

CREATE TYPE v2.booking_type_enum AS ENUM (
    'Instant',
    'Schedule',
    'FollowUp'
);


--
-- Name: consultation_type_enum; Type: TYPE; Schema: v2; Owner: -
--

CREATE TYPE v2.consultation_type_enum AS ENUM (
    'video',
    'voice',
    'chat'
);


--
-- Name: fhir_appointment_status_enum; Type: TYPE; Schema: v2; Owner: -
--

CREATE TYPE v2.fhir_appointment_status_enum AS ENUM (
    'PROPOSED',
    'PENDING',
    'BOOKED',
    'ARRIVED',
    'FULFILLED',
    'CANCELLED',
    'NOSHOW',
    'ENTERED_IN_ERROR'
);


--
-- Name: reservation_status_enum; Type: TYPE; Schema: v2; Owner: -
--

CREATE TYPE v2.reservation_status_enum AS ENUM (
    'RESERVED',
    'CONFIRMED',
    'RESERVE_EXPIRED',
    'CANCELLED'
);


--
-- Name: session_info_status_enum; Type: TYPE; Schema: v2; Owner: -
--

CREATE TYPE v2.session_info_status_enum AS ENUM (
    'EMPTY_ROOM_CREATED',
    'DOCTOR_JOINED',
    'PATIENT_JOINED',
    'ALL_PARTICIPANTS_JOINED',
    'ENDED'
);


--
-- Name: add_patient_id_verification(character varying, bigint); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.add_patient_id_verification(p_booking_id character varying, p_doctor_id bigint) RETURNS bigint
    LANGUAGE plpgsql
    AS $$
BEGIN
    RETURN v2.add_patient_verification_tx(p_booking_id, p_doctor_id);
END;
$$;


--
-- Name: add_patient_verification_tx(character varying, bigint); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.add_patient_verification_tx(p_appointment_id character varying, p_doctor_profile_id bigint) RETURNS bigint
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: cancel_appointment(character varying, bigint); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.cancel_appointment(p_booking_id character varying, p_doctor_id bigint) RETURNS bigint
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: create_appointment_internal(integer, integer, integer, integer, integer, integer, integer, v2.booking_type_enum, v2.consultation_type_enum, timestamp with time zone, timestamp with time zone, v2.fhir_appointment_status_enum, bigint, character varying, jsonb, character varying, text, character varying, character varying); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.create_appointment_internal(p_patient_account_id integer, p_patient_profile_id integer, p_doctor_account_id integer, p_doctor_profile_id integer, p_biz_unit_id integer, p_biz_center_id integer, p_tenant_id integer, p_booking_type v2.booking_type_enum, p_consultation_channel v2.consultation_type_enum, p_appointment_start timestamp with time zone, p_appointment_end timestamp with time zone, p_appointment_status v2.fhir_appointment_status_enum, p_payment_tx_id bigint, p_payment_tx_ref_id character varying DEFAULT NULL::character varying, p_payment_channels jsonb DEFAULT NULL::jsonb, p_parent_appointment_id character varying DEFAULT NULL::character varying, p_prescreen_data text DEFAULT '{}'::text, p_prescreen_data_type character varying DEFAULT 'RAW_JSON'::character varying, p_appointment_no character varying DEFAULT NULL::character varying) RETURNS TABLE(booking_id character varying, appointment_id character varying)
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_booking_id varchar(20);
    v_prescreen_data_id integer;
    v_payment_tx_ref_id varchar(255);
BEGIN
    v_booking_id := COALESCE(NULLIF(BTRIM(p_appointment_no), ''), v2.generate_booking_id());

    -- Use provided payment_tx_ref_id or auto-generate.
    IF p_payment_tx_ref_id IS NOT NULL THEN
        v_payment_tx_ref_id := p_payment_tx_ref_id;
    ELSE
        v_payment_tx_ref_id := v2.generate_uuid_v7()::varchar;
    END IF;

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
        p_doctor_account_id,
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
        p_appointment_status,
        p_appointment_start,
        p_appointment_end - p_appointment_start,
        p_appointment_end,
        false
    );

    INSERT INTO v2.appointment_payment_transaction (
        appointment_id,
        payment_tx_id,
        payment_tx_ref_id,
        payment_channels
    ) VALUES (
        v_booking_id,
        p_payment_tx_id,
        v_payment_tx_ref_id,
        p_payment_channels
    );

    RETURN QUERY
    SELECT
        r.booking_id,
        a.appointment_id
    FROM v2.reservation r
    INNER JOIN v2.appointment a ON a.appointment_id = r.booking_id
    WHERE r.booking_id = v_booking_id;
END;
$$;


--
-- Name: create_confirmed_appointment(integer, integer, integer, integer, integer, integer, integer, integer, v2.booking_type_enum, v2.consultation_type_enum, timestamp with time zone, timestamp with time zone, text, character varying, character varying, jsonb); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.create_confirmed_appointment(p_patient_account_id integer, p_patient_profile_id integer, p_doctor_id integer, p_doctor_account_id integer, p_doctor_profile_id integer, p_biz_unit_id integer, p_biz_center_id integer, p_tenant_id integer, p_booking_type v2.booking_type_enum, p_consultation_channel v2.consultation_type_enum, p_appointment_start timestamp with time zone, p_appointment_end timestamp with time zone, p_prescreen_data text, p_prescreen_data_type character varying, p_parent_appointment_id character varying DEFAULT NULL::character varying, p_payment_channels jsonb DEFAULT NULL::jsonb) RETURNS TABLE(booking_id character varying, appointment_id character varying, reservation_status v2.reservation_status_enum, appointment_status v2.fhir_appointment_status_enum, prescreen_data_id integer)
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: create_follow_up_appointment(character varying, timestamp with time zone, interval, v2.appointment_type_enum); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.create_follow_up_appointment(p_parent_appointment_id character varying, p_appointment_start timestamp with time zone, p_consult_duration interval, p_appointment_type v2.appointment_type_enum DEFAULT 'ROUTINE'::v2.appointment_type_enum) RETURNS jsonb
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: create_if_not_existing_summary_note(character varying, text, character varying, text, jsonb, bigint); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.create_if_not_existing_summary_note(p_appointment_id character varying, p_encrypted_data text, p_encrypted_data_type character varying, p_note_to_staff text, p_icd10_codes jsonb, p_prescription_id bigint DEFAULT NULL::bigint) RETURNS jsonb
    LANGUAGE plpgsql
    AS $$
DECLARE
    v_summary_note_id bigint;
    v_patient_account_id integer;
    v_patient_profile_id integer;
    v_biz_unit_id integer;
    v_biz_center_id integer;
    v_tenant_id integer;
    v_created boolean := true;
    v_is_follow_up boolean;
BEGIN
    SELECT parent_appointment_id IS NOT NULL INTO v_is_follow_up
    FROM v2.appointment
    WHERE appointment_id = p_appointment_id;

    IF v_is_follow_up THEN
        INSERT INTO v2.doctor_summary_note (
            appointment_id,
            encrypted_data,
            encrypted_data_type,
            note_to_staff,
            icd10_codes,
            prescription_id
        ) VALUES (
            p_appointment_id,
            p_encrypted_data,
            p_encrypted_data_type,
            p_note_to_staff,
            p_icd10_codes,
            p_prescription_id
        )
        ON CONFLICT (appointment_id) DO UPDATE SET
            encrypted_data = EXCLUDED.encrypted_data,
            encrypted_data_type = EXCLUDED.encrypted_data_type,
            note_to_staff = EXCLUDED.note_to_staff,
            icd10_codes = EXCLUDED.icd10_codes,
            prescription_id = EXCLUDED.prescription_id
        RETURNING summary_note_id INTO v_summary_note_id;

        IF v_summary_note_id IS NULL THEN
            v_created := false;
        END IF;
    ELSE
        INSERT INTO v2.doctor_summary_note (
            appointment_id,
            encrypted_data,
            encrypted_data_type,
            note_to_staff,
            icd10_codes,
            prescription_id
        ) VALUES (
            p_appointment_id,
            p_encrypted_data,
            p_encrypted_data_type,
            p_note_to_staff,
            p_icd10_codes,
            p_prescription_id
        )
        ON CONFLICT (appointment_id) DO NOTHING
        RETURNING summary_note_id INTO v_summary_note_id;

        IF v_summary_note_id IS NULL THEN
            v_created := false;
        END IF;
    END IF;

    IF v_summary_note_id IS NULL THEN
        SELECT dsn.summary_note_id INTO v_summary_note_id
        FROM v2.doctor_summary_note dsn
        WHERE dsn.appointment_id = p_appointment_id;
    END IF;

    -- Terminal transition: submitting the summary fulfils the appointment.
    UPDATE v2.appointment
    SET appointment_status = 'FULFILLED'::v2.fhir_appointment_status_enum,
        modified_at = NOW()
    WHERE appointment_id = p_appointment_id;

    SELECT
        r.patient_account_id,
        r.patient_profile_id,
        r.biz_unit_id,
        r.biz_center_id,
        r.tenant_id
    INTO
        v_patient_account_id,
        v_patient_profile_id,
        v_biz_unit_id,
        v_biz_center_id,
        v_tenant_id
    FROM v2.reservation r
    WHERE r.booking_id = p_appointment_id;

    RETURN jsonb_build_object(
        'created', v_created,
        'summaryNoteId', COALESCE(v_summary_note_id, 0),
        'patientAccountId', COALESCE(v_patient_account_id, 0),
        'userProfileId', COALESCE(v_patient_profile_id, 0),
        'tenantId', COALESCE(v_tenant_id, 1),
        'bizUnitId', COALESCE(v_biz_unit_id, 0),
        'bizCenterId', COALESCE(v_biz_center_id, 0)
    );
END;
$$;


--
-- Name: create_reservation(integer, integer, integer, integer, integer, integer, integer, integer, v2.booking_type_enum, v2.consultation_type_enum, timestamp with time zone, integer); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.create_reservation(p_patient_account_id integer, p_patient_profile_id integer, p_doctor_id integer, p_doctor_account_id integer, p_doctor_profile_id integer, p_biz_unit_id integer, p_biz_center_id integer, p_tenant_id integer, p_booking_type v2.booking_type_enum, p_consultation_channel v2.consultation_type_enum, p_reserved_from timestamp with time zone, p_ttl_seconds integer) RETURNS TABLE(booking_id character varying)
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: end_active_session(character varying, bigint); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.end_active_session(p_appointment_id character varying, p_doctor_profile_id bigint) RETURNS bigint
    LANGUAGE plpgsql
    AS $$
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


--
-- Name: generate_booking_id(); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.generate_booking_id() RETURNS character varying
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: generate_timeslots_for_date(date, time without time zone, time without time zone, integer, integer); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.generate_timeslots_for_date(p_date date, p_working_start_time time without time zone, p_working_end_time time without time zone, p_slot_duration_minutes integer, p_gap_minutes integer) RETURNS TABLE(slot_date date, slot_start_time time without time zone, slot_end_time time without time zone)
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: generate_uuid_v7(); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.generate_uuid_v7() RETURNS uuid
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: get_appointment_chain(character varying); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.get_appointment_chain(p_appointment_id character varying) RETURNS jsonb
    LANGUAGE plpgsql STABLE
    AS $$
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
$$;


--
-- Name: get_appointment_detail(character varying); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.get_appointment_detail(p_booking_id character varying) RETURNS TABLE(booking_id character varying, appointment_start timestamp with time zone, appointment_end timestamp with time zone, appointment_status v2.fhir_appointment_status_enum, booking_type v2.booking_type_enum, consultation_channel v2.consultation_type_enum, patient_account_id integer, patient_profile_id integer, doctor_account_id integer, doctor_profile_id integer, prescreen_data text, prescreen_data_type character varying, payment_tx_id bigint, payment_tx_ref_id character varying)
    LANGUAGE plpgsql
    AS $$
BEGIN
    RETURN QUERY
    SELECT
        r.booking_id,
        r.appointment_start,
        r.appointment_end,
        a.appointment_status,
        r.booking_type,
        r.consultation_channel,
        r.patient_account_id,
        r.patient_profile_id,
        r.doctor_account_id,
        r.doctor_profile_id,
        ps.prescreen_data,
        ps.prescreen_data_type,
        pay.payment_tx_id,
        pay.payment_tx_ref_id
    FROM v2.reservation r
    INNER JOIN v2.appointment a ON a.appointment_id = r.booking_id
    INNER JOIN v2.patient_prescreen ps ON ps.booking_id = r.booking_id
    INNER JOIN v2.appointment_payment_transaction pay ON pay.appointment_id = r.booking_id
    WHERE r.booking_id = p_booking_id;
END;
$$;


--
-- Name: get_available_timeslots(integer, date, date, time without time zone, time without time zone, integer, integer); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.get_available_timeslots(p_doctor_id integer, p_start_date date, p_end_date date, p_working_start_time time without time zone DEFAULT '09:00:00'::time without time zone, p_working_end_time time without time zone DEFAULT '17:00:00'::time without time zone, p_slot_duration_minutes integer DEFAULT 30, p_gap_minutes integer DEFAULT 5) RETURNS TABLE(slot_id bigint, slot_date date, slot_start_time time without time zone, slot_end_time time without time zone, slot_start_timestamptz timestamp with time zone, slot_end_timestamptz timestamp with time zone)
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: get_consultation_session(character varying, integer); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.get_consultation_session(p_appointment_id character varying, p_user_profile_id integer) RETURNS TABLE(appointment_id character varying, session_provider_name text, session_data jsonb, appointment_status v2.fhir_appointment_status_enum, patient_profile_id bigint, doctor_profile_id bigint, consultation_start_time bigint, consultation_end_time bigint, consultation_channel v2.consultation_type_enum, payment_channels jsonb, is_facial_verified boolean)
    LANGUAGE plpgsql STABLE
    AS $$
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


--
-- Name: get_doctor_reservations(integer, date, date); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.get_doctor_reservations(p_doctor_id integer, p_start_date date, p_end_date date) RETURNS TABLE(booking_id character varying, appointment_start timestamp with time zone, appointment_end timestamp with time zone)
    LANGUAGE plpgsql STABLE
    AS $$
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
$$;


--
-- Name: get_reservation_by_id(character varying); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.get_reservation_by_id(p_reservation_id character varying) RETURNS TABLE(booking_id character varying, patient_account_id integer, patient_profile_id integer, doctor_id integer, doctor_account_id integer, doctor_profile_id integer, biz_unit_id integer, biz_center_id integer, tenant_id integer, reservation_status v2.reservation_status_enum, reserved_until timestamp with time zone, booking_type v2.booking_type_enum, consultation_channel v2.consultation_type_enum, appointment_start timestamp with time zone, appointment_end timestamp with time zone)
    LANGUAGE plpgsql STABLE
    AS $$
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
$$;


--
-- Name: get_session_details(character varying); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.get_session_details(p_appointment_id character varying) RETURNS TABLE(appointment_id character varying, booking_id character varying, patient_account_id integer, patient_profile_id integer, tenant_id integer, doctor_id integer, doctor_profile_id integer, session_provider text, session_chat_id text)
    LANGUAGE plpgsql STABLE
    AS $$
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
$$;


--
-- Name: list_fulfilled_appointments_by_patient(integer, integer); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.list_fulfilled_appointments_by_patient(p_account_id integer, p_profile_id integer) RETURNS TABLE(booking_id character varying, appointment_start timestamp with time zone, appointment_end timestamp with time zone, doctor_account_id integer, doctor_profile_id integer)
    LANGUAGE plpgsql
    AS $$
BEGIN
    RETURN QUERY
    SELECT
        r.booking_id,
        r.appointment_start,
        r.appointment_end,
        r.doctor_account_id,
        r.doctor_profile_id
    FROM v2.reservation r
    INNER JOIN v2.appointment a ON a.appointment_id = r.booking_id
    WHERE r.patient_account_id = p_account_id
      AND (p_profile_id IS NULL OR r.patient_profile_id = p_profile_id)
      AND a.appointment_status = 'FULFILLED'::v2.fhir_appointment_status_enum
    ORDER BY r.appointment_start DESC
    LIMIT 50;
END;
$$;


--
-- Name: mark_appointment_has_follow_up(character varying); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.mark_appointment_has_follow_up(p_appointment_id character varying) RETURNS boolean
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: update_modified_at_column(); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.update_modified_at_column() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    NEW.modified_at = NOW();
    RETURN NEW;
END;
$$;


--
-- Name: upsert_payment_transaction(character varying, bigint, character varying, jsonb); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.upsert_payment_transaction(p_booking_id character varying, p_payment_tx_id bigint, p_payment_tx_ref_id character varying, p_payment_channels jsonb) RETURNS TABLE(booking_id character varying, patient_account_id integer, patient_profile_id integer, tenant_id integer, doctor_id integer, biz_unit_id integer, booking_type text, consultation_channel text, consultation_start_time bigint, consultation_duration_in_second integer, symptoms text, should_publish_consultation_booked boolean)
    LANGUAGE plpgsql
    AS $$
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
$$;


--
-- Name: upsert_session_info(character varying, jsonb); Type: FUNCTION; Schema: v2; Owner: -
--

CREATE FUNCTION v2.upsert_session_info(p_appointment_id character varying, p_session_data jsonb) RETURNS void
    LANGUAGE plpgsql
    AS $$
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
$$;


SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: appointment; Type: TABLE; Schema: v2; Owner: -
--

CREATE TABLE v2.appointment (
    appointment_id character varying(20) NOT NULL,
    booking_id character varying(20) NOT NULL,
    prescreen_data_id integer NOT NULL,
    parent_appointment_id character varying(20),
    appointment_status v2.fhir_appointment_status_enum DEFAULT 'PENDING'::v2.fhir_appointment_status_enum NOT NULL,
    appointment_start timestamp with time zone NOT NULL,
    consult_duration interval NOT NULL,
    appointment_end timestamp with time zone NOT NULL,
    has_follow_up boolean DEFAULT false NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    modified_at timestamp with time zone DEFAULT now() NOT NULL,
    appointment_type v2.appointment_type_enum DEFAULT 'ROUTINE'::v2.appointment_type_enum
);


--
-- Name: appointment_cancellation; Type: TABLE; Schema: v2; Owner: -
--

CREATE TABLE v2.appointment_cancellation (
    appointment_id bigint NOT NULL,
    cancel_reason jsonb,
    created_at timestamp with time zone
);


--
-- Name: appointment_event_publication; Type: TABLE; Schema: v2; Owner: -
--

CREATE TABLE v2.appointment_event_publication (
    appointment_id character varying(20) NOT NULL,
    event_type character varying(255) NOT NULL,
    publication_status character varying(32) DEFAULT 'PENDING'::character varying NOT NULL,
    published_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    modified_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: appointment_facial_upload; Type: TABLE; Schema: v2; Owner: -
--

CREATE TABLE v2.appointment_facial_upload (
    appointment_id character varying(20) NOT NULL,
    user_profile_id integer NOT NULL,
    user_account_id integer NOT NULL,
    object_url character varying(250) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: appointment_payment_transaction; Type: TABLE; Schema: v2; Owner: -
--

CREATE TABLE v2.appointment_payment_transaction (
    appointment_id character varying(20) NOT NULL,
    payment_tx_ref_id character varying(255) NOT NULL,
    payment_channels jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    cancelled_at timestamp with time zone,
    modified_at timestamp with time zone DEFAULT now() NOT NULL,
    payment_tx_id bigint DEFAULT 0 NOT NULL
);


--
-- Name: doctor_summary_note; Type: TABLE; Schema: v2; Owner: -
--

CREATE TABLE v2.doctor_summary_note (
    summary_note_id integer NOT NULL,
    appointment_id character varying(20) NOT NULL,
    encrypted_data text NOT NULL,
    encrypted_data_type character varying(120) DEFAULT 'DoctorSummaryNoteV1'::character varying NOT NULL,
    note_to_staff text,
    icd10_codes jsonb DEFAULT '{}'::jsonb NOT NULL,
    tenant_id integer DEFAULT 1 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    modified_at timestamp with time zone DEFAULT now() NOT NULL,
    prescription_id bigint
);


--
-- Name: doctor_summary_note_summary_note_id_seq; Type: SEQUENCE; Schema: v2; Owner: -
--

CREATE SEQUENCE v2.doctor_summary_note_summary_note_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: doctor_summary_note_summary_note_id_seq; Type: SEQUENCE OWNED BY; Schema: v2; Owner: -
--

ALTER SEQUENCE v2.doctor_summary_note_summary_note_id_seq OWNED BY v2.doctor_summary_note.summary_note_id;


--
-- Name: patient_id_verification_transaction; Type: TABLE; Schema: v2; Owner: -
--

CREATE TABLE v2.patient_id_verification_transaction (
    appointment_id character varying(20) NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: patient_prescreen; Type: TABLE; Schema: v2; Owner: -
--

CREATE TABLE v2.patient_prescreen (
    prescreen_id integer NOT NULL,
    booking_id character varying(20) NOT NULL,
    prescreen_data text NOT NULL,
    prescreen_data_type character varying(255) NOT NULL,
    user_account_id integer NOT NULL,
    user_profile_id integer NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    modified_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: patient_prescreen_prescreen_id_seq; Type: SEQUENCE; Schema: v2; Owner: -
--

CREATE SEQUENCE v2.patient_prescreen_prescreen_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: patient_prescreen_prescreen_id_seq; Type: SEQUENCE OWNED BY; Schema: v2; Owner: -
--

ALTER SEQUENCE v2.patient_prescreen_prescreen_id_seq OWNED BY v2.patient_prescreen.prescreen_id;


--
-- Name: reservation; Type: TABLE; Schema: v2; Owner: -
--

CREATE TABLE v2.reservation (
    booking_id character varying(20) NOT NULL,
    patient_account_id integer NOT NULL,
    patient_profile_id integer NOT NULL,
    doctor_id integer NOT NULL,
    doctor_account_id integer NOT NULL,
    doctor_profile_id integer NOT NULL,
    biz_unit_id integer,
    biz_center_id integer,
    tenant_id integer DEFAULT 1 NOT NULL,
    reservation_status v2.reservation_status_enum DEFAULT 'RESERVED'::v2.reservation_status_enum NOT NULL,
    reserved_until timestamp with time zone NOT NULL,
    booking_type v2.booking_type_enum NOT NULL,
    consultation_channel v2.consultation_type_enum NOT NULL,
    appointment_start timestamp with time zone NOT NULL,
    appointment_end timestamp with time zone NOT NULL,
    cancelled_at timestamp with time zone,
    deleted_at timestamp with time zone,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    modified_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: reservation_booking_id_seq; Type: SEQUENCE; Schema: v2; Owner: -
--

CREATE SEQUENCE v2.reservation_booking_id_seq
    START WITH 100000
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: session_info; Type: TABLE; Schema: v2; Owner: -
--

CREATE TABLE v2.session_info (
    session_id integer NOT NULL,
    appointment_id character varying(20) NOT NULL,
    session_provider character varying(255) NOT NULL,
    session_status v2.session_info_status_enum DEFAULT 'EMPTY_ROOM_CREATED'::v2.session_info_status_enum,
    session_data jsonb,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    modified_at timestamp with time zone DEFAULT now() NOT NULL
);


--
-- Name: session_info_session_id_seq; Type: SEQUENCE; Schema: v2; Owner: -
--

CREATE SEQUENCE v2.session_info_session_id_seq
    AS integer
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: session_info_session_id_seq; Type: SEQUENCE OWNED BY; Schema: v2; Owner: -
--

ALTER SEQUENCE v2.session_info_session_id_seq OWNED BY v2.session_info.session_id;


--
-- Name: doctor_summary_note summary_note_id; Type: DEFAULT; Schema: v2; Owner: -
--

ALTER TABLE ONLY v2.doctor_summary_note ALTER COLUMN summary_note_id SET DEFAULT nextval('v2.doctor_summary_note_summary_note_id_seq'::regclass);


--
-- Name: patient_prescreen prescreen_id; Type: DEFAULT; Schema: v2; Owner: -
--

ALTER TABLE ONLY v2.patient_prescreen ALTER COLUMN prescreen_id SET DEFAULT nextval('v2.patient_prescreen_prescreen_id_seq'::regclass);


--
-- Name: session_info session_id; Type: DEFAULT; Schema: v2; Owner: -
--

ALTER TABLE ONLY v2.session_info ALTER COLUMN session_id SET DEFAULT nextval('v2.session_info_session_id_seq'::regclass);


--
-- Name: appointment_event_publication appointment_event_publication_pkey; Type: CONSTRAINT; Schema: v2; Owner: -
--

ALTER TABLE ONLY v2.appointment_event_publication
    ADD CONSTRAINT appointment_event_publication_pkey PRIMARY KEY (appointment_id, event_type);


--
-- Name: appointment_facial_upload appointment_facial_upload_pkey; Type: CONSTRAINT; Schema: v2; Owner: -
--

ALTER TABLE ONLY v2.appointment_facial_upload
    ADD CONSTRAINT appointment_facial_upload_pkey PRIMARY KEY (appointment_id);


--
-- Name: appointment_payment_transaction appointment_payment_transaction_appointment_id_key; Type: CONSTRAINT; Schema: v2; Owner: -
--

ALTER TABLE ONLY v2.appointment_payment_transaction
    ADD CONSTRAINT appointment_payment_transaction_appointment_id_key UNIQUE (appointment_id);


--
-- Name: appointment appointment_pkey; Type: CONSTRAINT; Schema: v2; Owner: -
--

ALTER TABLE ONLY v2.appointment
    ADD CONSTRAINT appointment_pkey PRIMARY KEY (appointment_id);


--
-- Name: doctor_summary_note doctor_summary_note_pkey; Type: CONSTRAINT; Schema: v2; Owner: -
--

ALTER TABLE ONLY v2.doctor_summary_note
    ADD CONSTRAINT doctor_summary_note_pkey PRIMARY KEY (summary_note_id);


--
-- Name: patient_id_verification_transaction patient_id_verification_transaction_pk; Type: CONSTRAINT; Schema: v2; Owner: -
--

ALTER TABLE ONLY v2.patient_id_verification_transaction
    ADD CONSTRAINT patient_id_verification_transaction_pk UNIQUE (appointment_id);


--
-- Name: patient_prescreen patient_prescreen_pkey; Type: CONSTRAINT; Schema: v2; Owner: -
--

ALTER TABLE ONLY v2.patient_prescreen
    ADD CONSTRAINT patient_prescreen_pkey PRIMARY KEY (prescreen_id);


--
-- Name: reservation reservation_pkey; Type: CONSTRAINT; Schema: v2; Owner: -
--

ALTER TABLE ONLY v2.reservation
    ADD CONSTRAINT reservation_pkey PRIMARY KEY (booking_id);


--
-- Name: session_info session_info_appointment_id_key; Type: CONSTRAINT; Schema: v2; Owner: -
--

ALTER TABLE ONLY v2.session_info
    ADD CONSTRAINT session_info_appointment_id_key UNIQUE (appointment_id);


--
-- Name: session_info session_info_pkey; Type: CONSTRAINT; Schema: v2; Owner: -
--

ALTER TABLE ONLY v2.session_info
    ADD CONSTRAINT session_info_pkey PRIMARY KEY (session_id);


--
-- Name: doctor_summary_note uk_doctor_summary_note_appointment_id; Type: CONSTRAINT; Schema: v2; Owner: -
--

ALTER TABLE ONLY v2.doctor_summary_note
    ADD CONSTRAINT uk_doctor_summary_note_appointment_id UNIQUE (appointment_id);


--
-- Name: idx_appointment_cancellation_appointment_id; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_appointment_cancellation_appointment_id ON v2.appointment_cancellation USING btree (appointment_id);


--
-- Name: idx_appointment_facial_upload_user_account_id; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_appointment_facial_upload_user_account_id ON v2.appointment_facial_upload USING btree (user_account_id);


--
-- Name: idx_appointment_facial_upload_user_profile_id; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_appointment_facial_upload_user_profile_id ON v2.appointment_facial_upload USING btree (user_profile_id);


--
-- Name: idx_appointment_parent_appointment_id; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_appointment_parent_appointment_id ON v2.appointment USING btree (parent_appointment_id);


--
-- Name: idx_appointment_payment_transaction_appointment_id; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_appointment_payment_transaction_appointment_id ON v2.appointment_payment_transaction USING btree (appointment_id);


--
-- Name: idx_appointment_payment_tx_payment_tx_id; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_appointment_payment_tx_payment_tx_id ON v2.appointment_payment_transaction USING btree (payment_tx_id);


--
-- Name: idx_appointment_payment_tx_payment_tx_ref_id; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_appointment_payment_tx_payment_tx_ref_id ON v2.appointment_payment_transaction USING btree (payment_tx_ref_id);


--
-- Name: idx_appointment_status; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_appointment_status ON v2.appointment USING btree (appointment_status);


--
-- Name: idx_doctor_summary_note_appointment_id; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_doctor_summary_note_appointment_id ON v2.doctor_summary_note USING btree (appointment_id);


--
-- Name: idx_doctor_summary_note_icd10_codes; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_doctor_summary_note_icd10_codes ON v2.doctor_summary_note USING gin (icd10_codes);


--
-- Name: idx_doctor_summary_note_tenant_id; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_doctor_summary_note_tenant_id ON v2.doctor_summary_note USING btree (tenant_id);


--
-- Name: idx_patient_prescreen_booking_id; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_patient_prescreen_booking_id ON v2.patient_prescreen USING btree (booking_id);


--
-- Name: idx_reservation_appointment_end; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_reservation_appointment_end ON v2.reservation USING btree (appointment_end);


--
-- Name: idx_reservation_appointment_start; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_reservation_appointment_start ON v2.reservation USING btree (appointment_start);


--
-- Name: idx_reservation_booking_type; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_reservation_booking_type ON v2.reservation USING btree (booking_type);


--
-- Name: idx_reservation_doctor_account_id; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_reservation_doctor_account_id ON v2.reservation USING btree (doctor_account_id);


--
-- Name: idx_reservation_doctor_profile_id; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_reservation_doctor_profile_id ON v2.reservation USING btree (doctor_profile_id);


--
-- Name: idx_reservation_patient_account_id; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_reservation_patient_account_id ON v2.reservation USING btree (patient_account_id);


--
-- Name: idx_reservation_patient_profile_id; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_reservation_patient_profile_id ON v2.reservation USING btree (patient_profile_id);


--
-- Name: idx_reservation_tenant_id; Type: INDEX; Schema: v2; Owner: -
--

CREATE INDEX idx_reservation_tenant_id ON v2.reservation USING btree (tenant_id);


--
-- Name: appointment_event_publication update_appointment_event_publication_modified_at; Type: TRIGGER; Schema: v2; Owner: -
--

CREATE TRIGGER update_appointment_event_publication_modified_at BEFORE UPDATE ON v2.appointment_event_publication FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();


--
-- Name: appointment update_appointment_modified_at; Type: TRIGGER; Schema: v2; Owner: -
--

CREATE TRIGGER update_appointment_modified_at BEFORE UPDATE ON v2.appointment FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();


--
-- Name: appointment_payment_transaction update_appointment_payment_tx_modified_at; Type: TRIGGER; Schema: v2; Owner: -
--

CREATE TRIGGER update_appointment_payment_tx_modified_at BEFORE UPDATE ON v2.appointment_payment_transaction FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();


--
-- Name: doctor_summary_note update_doctor_summary_note_modified_at; Type: TRIGGER; Schema: v2; Owner: -
--

CREATE TRIGGER update_doctor_summary_note_modified_at BEFORE UPDATE ON v2.doctor_summary_note FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();


--
-- Name: patient_prescreen update_patient_prescreen_modified_at; Type: TRIGGER; Schema: v2; Owner: -
--

CREATE TRIGGER update_patient_prescreen_modified_at BEFORE UPDATE ON v2.patient_prescreen FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();


--
-- Name: reservation update_reservation_modified_at; Type: TRIGGER; Schema: v2; Owner: -
--

CREATE TRIGGER update_reservation_modified_at BEFORE UPDATE ON v2.reservation FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();


--
-- Name: session_info update_session_info_modified_at; Type: TRIGGER; Schema: v2; Owner: -
--

CREATE TRIGGER update_session_info_modified_at BEFORE UPDATE ON v2.session_info FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();


--
