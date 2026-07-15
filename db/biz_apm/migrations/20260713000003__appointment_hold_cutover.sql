-- sqlfluff:dialect:postgres
--
-- ADR 0006 cutover.  `v2.reservation` is deliberately not read or written by
-- the functions below.  The thirteen-argument create_reservation function is
-- retained solely as an old-pod SQL adapter during the rollout.

CREATE EXTENSION IF NOT EXISTS btree_gist;

ALTER TABLE v2.appointment_hold
    ADD COLUMN IF NOT EXISTS booking_id varchar(20);

UPDATE v2.appointment_hold
SET booking_id = COALESCE(booking_id, left(replace(hold_id::text, '-', ''), 20))
WHERE booking_id IS NULL;

ALTER TABLE v2.appointment_hold
    ALTER COLUMN booking_id SET NOT NULL;

-- A Hold is the financial offer boundary.  The quote is copied from the
-- DoctorApp projection while the projection row is locked by the canonical
-- writer; payment confirmation compares the signed effective service-config
-- coordinate (profileVersion when present, otherwise occurredAt) against this
-- immutable snapshot, never against a subsequently changed profile.
ALTER TABLE v2.appointment_hold
    ADD COLUMN IF NOT EXISTS quoted_amount numeric(18, 2),
    ADD COLUMN IF NOT EXISTS quoted_currency varchar(16),
    ADD COLUMN IF NOT EXISTS quoted_profile_version bigint,
    ADD COLUMN IF NOT EXISTS quoted_service_config_version bigint;

-- Appointment owns the clinical identity after booking.  These columns are
-- deliberately duplicated from a consumed Hold so direct/internal
-- Appointments have the same read model and never need v2.reservation.
ALTER TABLE v2.appointment
    ADD COLUMN IF NOT EXISTS patient_account_id integer,
    ADD COLUMN IF NOT EXISTS patient_profile_id integer,
    ADD COLUMN IF NOT EXISTS doctor_id integer,
    ADD COLUMN IF NOT EXISTS doctor_account_id integer,
    ADD COLUMN IF NOT EXISTS doctor_profile_id integer,
    ADD COLUMN IF NOT EXISTS biz_unit_id integer,
    ADD COLUMN IF NOT EXISTS biz_center_id integer,
    ADD COLUMN IF NOT EXISTS tenant_id integer,
    ADD COLUMN IF NOT EXISTS booking_type v2.booking_type_enum,
    ADD COLUMN IF NOT EXISTS consultation_channel v2.consultation_type_enum;

-- A confirmed payment records the exact immutable Hold quote it consumed.
-- This makes replay validation independent of a mutable DoctorApp projection.
ALTER TABLE v2.appointment_payment_transaction
    ADD COLUMN IF NOT EXISTS payment_amount numeric(18, 2),
    ADD COLUMN IF NOT EXISTS payment_currency varchar(16),
    ADD COLUMN IF NOT EXISTS consultation_config_version bigint,
    ADD COLUMN IF NOT EXISTS payment_module_id integer,
    ADD COLUMN IF NOT EXISTS booked_at bigint;

-- Do not silently deploy a supposedly idempotent event path over historical
-- duplicates.  Operators must repair them before the cutover can proceed.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM v2.event_outbox
        WHERE aggregate_id IS NOT NULL AND event_type = 'ConsultationBooked'
        GROUP BY aggregate_id, event_type HAVING COUNT(*) > 1
    ) THEN
        RAISE EXCEPTION 'APMv2 cutover blocked: duplicate ConsultationBooked outbox rows require repair'
            USING ERRCODE = 'P1007';
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS idx_event_outbox_consultation_booked_replay
    ON v2.event_outbox (aggregate_id)
    WHERE aggregate_id IS NOT NULL AND event_type = 'ConsultationBooked';

-- The target-model migration temporarily installed a second overlap ledger.
-- Capacity is now exclusively represented by doctor_occupancy.
ALTER TABLE v2.appointment_hold
    DROP CONSTRAINT IF EXISTS appointment_hold_active_doctor_overlap_excl;
DROP INDEX IF EXISTS v2.idx_appointment_hold_active_range;

DO $$ BEGIN
    ALTER TABLE v2.appointment_hold
        ADD CONSTRAINT appointment_hold_booking_id_key UNIQUE (booking_id);
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM v2.appointment GROUP BY booking_id HAVING COUNT(*) > 1) THEN
        RAISE EXCEPTION 'APMv2 cutover blocked: duplicate appointment.booking_id rows require repair'
            USING ERRCODE = 'P1007';
    END IF;
    IF EXISTS (
        SELECT 1 FROM v2.appointment WHERE source_hold_id IS NOT NULL
        GROUP BY source_hold_id HAVING COUNT(*) > 1
    ) THEN
        RAISE EXCEPTION 'APMv2 cutover blocked: duplicate appointment.source_hold_id rows require repair'
            USING ERRCODE = 'P1007';
    END IF;
END $$;
CREATE UNIQUE INDEX IF NOT EXISTS idx_appointment_booking_id_unique
    ON v2.appointment (booking_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_appointment_source_hold_id_unique
    ON v2.appointment (source_hold_id) WHERE source_hold_id IS NOT NULL;

-- Internal aggregate IDs must never be public booking IDs.  UUID-v7 is
-- already used in this schema; the compact value keeps the historical varchar
-- contract without conflating the two identities.
CREATE OR REPLACE FUNCTION v2.generate_appointment_id() RETURNS varchar(20) AS $$
BEGIN
    RETURN left('apt_' || replace(v2.generate_uuid_v7()::text, '-', ''), 20);
END;
$$ LANGUAGE plpgsql VOLATILE;

DO $$ BEGIN
    CREATE TYPE v2.doctor_occupancy_status_enum AS ENUM ('ACTIVE', 'RELEASED');
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

CREATE TABLE IF NOT EXISTS v2.doctor_occupancy (
    occupancy_id uuid PRIMARY KEY DEFAULT v2.generate_uuid_v7(),
    doctor_profile_id integer NOT NULL,
    starts_at timestamptz NOT NULL,
    ends_at timestamptz NOT NULL,
    occupancy_status v2.doctor_occupancy_status_enum NOT NULL DEFAULT 'ACTIVE',
    hold_id uuid REFERENCES v2.appointment_hold (hold_id),
    appointment_id varchar(20) REFERENCES v2.appointment (appointment_id),
    released_at timestamptz,
    release_reason text,
    created_at timestamptz NOT NULL DEFAULT now(),
    modified_at timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT doctor_occupancy_time_range CHECK (starts_at < ends_at),
    CONSTRAINT doctor_occupancy_one_owner CHECK (
        (hold_id IS NOT NULL)::integer + (appointment_id IS NOT NULL)::integer = 1
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_doctor_occupancy_hold_id
    ON v2.doctor_occupancy (hold_id) WHERE hold_id IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_doctor_occupancy_appointment_id
    ON v2.doctor_occupancy (appointment_id) WHERE appointment_id IS NOT NULL;

DO $$ BEGIN
    ALTER TABLE v2.doctor_occupancy
        ADD CONSTRAINT doctor_occupancy_active_overlap_excl
        EXCLUDE USING gist (
            doctor_profile_id WITH =,
            tstzrange(starts_at, ends_at, '[)') WITH &&
        ) WHERE (occupancy_status = 'ACTIVE'::v2.doctor_occupancy_status_enum);
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

DO $$ BEGIN
    CREATE TRIGGER update_doctor_occupancy_modified_at
    BEFORE UPDATE ON v2.doctor_occupancy
    FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();
EXCEPTION WHEN duplicate_object THEN NULL;
END $$;

-- Pre-launch/development backfill.  It is idempotent and may be rerun before
-- cutover; rows with an already-created booking_id win over legacy data.
INSERT INTO v2.appointment_hold (
    booking_id, idempotency_key, patient_account_id, patient_profile_id,
    doctor_id, doctor_account_id, doctor_profile_id, biz_unit_id,
    biz_center_id, tenant_id, booking_type, consultation_channel, starts_at,
    ends_at, expires_at, purpose_code, hold_status, released_at, release_reason,
    confirmed_appointment_id, created_at, modified_at
)
SELECT
    r.booking_id,
    'legacy-reservation:' || r.booking_id,
    r.patient_account_id, r.patient_profile_id, r.doctor_id,
    r.doctor_account_id, r.doctor_profile_id, r.biz_unit_id, r.biz_center_id,
    r.tenant_id, r.booking_type, r.consultation_channel, r.appointment_start,
    r.appointment_end, r.reserved_until, 'PATIENT_BOOKING',
    CASE r.reservation_status::text
        -- Legacy rows lack a trustworthy immutable quote/prescreen.  Pre-
        -- launch cutover explicitly expires unpaid reservations instead of
        -- allowing an unpriced active Hold to accept a payment.
        WHEN 'RESERVED' THEN 'EXPIRED'::v2.appointment_hold_status_enum
        WHEN 'RESERVE_EXPIRED' THEN 'EXPIRED'::v2.appointment_hold_status_enum
        ELSE 'RELEASED'::v2.appointment_hold_status_enum
    END,
    CASE WHEN r.reservation_status::text IN ('RESERVED', 'RESERVE_EXPIRED', 'CANCELLED', 'CONFIRMED')
         THEN COALESCE(r.cancelled_at, r.reserved_until) END,
    CASE WHEN r.reservation_status::text IN ('RESERVED', 'RESERVE_EXPIRED') THEN 'HoldExpired'
         WHEN r.reservation_status::text = 'CANCELLED' THEN 'HoldCancelled'
         WHEN r.reservation_status::text = 'CONFIRMED' THEN 'Booked' END,
    NULL,
    r.created_at, r.modified_at
FROM v2.reservation r
ON CONFLICT (booking_id) DO NOTHING;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM v2.appointment_hold
        WHERE idempotency_key LIKE 'legacy-reservation:%'
          AND hold_status = 'ACTIVE'::v2.appointment_hold_status_enum
    ) THEN
        RAISE EXCEPTION 'APMv2 cutover blocked: legacy ACTIVE Holds must be expired or rejected'
            USING ERRCODE = 'P1007';
    END IF;
END $$;

UPDATE v2.appointment a
SET source_hold_id = h.hold_id,
    source_hold_prescreen_id = COALESCE(a.source_hold_prescreen_id, h.source_prescreen_id),
    patient_account_id = COALESCE(a.patient_account_id, h.patient_account_id),
    patient_profile_id = COALESCE(a.patient_profile_id, h.patient_profile_id),
    doctor_id = COALESCE(a.doctor_id, h.doctor_id),
    doctor_account_id = COALESCE(a.doctor_account_id, h.doctor_account_id),
    doctor_profile_id = COALESCE(a.doctor_profile_id, h.doctor_profile_id),
    biz_unit_id = COALESCE(a.biz_unit_id, h.biz_unit_id),
    biz_center_id = COALESCE(a.biz_center_id, h.biz_center_id),
    tenant_id = COALESCE(a.tenant_id, h.tenant_id),
    booking_type = COALESCE(a.booking_type, h.booking_type),
    consultation_channel = COALESCE(a.consultation_channel, h.consultation_channel)
FROM v2.appointment_hold h
WHERE h.booking_id = a.booking_id
  AND a.source_hold_id IS NULL;

-- The system is pre-launch: legacy Appointments whose aggregate identity was
-- the public booking ID are re-keyed rather than preserved.  Booking
-- correlation remains in appointment.booking_id and every dependent relation
-- is moved to the new canonical Appointment ID before the primary key changes.
CREATE TEMP TABLE apmv2_legacy_appointment_ids ON COMMIT DROP AS
SELECT appointment_id AS old_appointment_id, v2.generate_appointment_id() AS new_appointment_id
FROM v2.appointment
WHERE appointment_id = booking_id;

UPDATE v2.session_info s SET appointment_id = m.new_appointment_id
FROM apmv2_legacy_appointment_ids m WHERE s.appointment_id = m.old_appointment_id;
UPDATE v2.appointment_payment_transaction p SET appointment_id = m.new_appointment_id
FROM apmv2_legacy_appointment_ids m WHERE p.appointment_id = m.old_appointment_id;
UPDATE v2.doctor_summary_note n SET appointment_id = m.new_appointment_id
FROM apmv2_legacy_appointment_ids m WHERE n.appointment_id = m.old_appointment_id;
UPDATE v2.appointment_facial_upload f SET appointment_id = m.new_appointment_id
FROM apmv2_legacy_appointment_ids m WHERE f.appointment_id = m.old_appointment_id;
UPDATE v2.patient_id_verification_transaction v SET appointment_id = m.new_appointment_id
FROM apmv2_legacy_appointment_ids m WHERE v.appointment_id = m.old_appointment_id;
UPDATE v2.doctor_occupancy o SET appointment_id = m.new_appointment_id
FROM apmv2_legacy_appointment_ids m WHERE o.appointment_id = m.old_appointment_id;
UPDATE v2.appointment child SET parent_appointment_id = m.new_appointment_id
FROM apmv2_legacy_appointment_ids m WHERE child.parent_appointment_id = m.old_appointment_id;
UPDATE v2.appointment a SET appointment_id = m.new_appointment_id
FROM apmv2_legacy_appointment_ids m WHERE a.appointment_id = m.old_appointment_id;
UPDATE v2.appointment_hold h SET confirmed_appointment_id = a.appointment_id
FROM v2.appointment a WHERE a.source_hold_id = h.hold_id;

INSERT INTO v2.doctor_occupancy (
    doctor_profile_id, starts_at, ends_at, occupancy_status, hold_id
)
SELECT h.doctor_profile_id, h.starts_at, h.ends_at,
       'ACTIVE'::v2.doctor_occupancy_status_enum, h.hold_id
FROM v2.appointment_hold h
LEFT JOIN v2.appointment a ON a.source_hold_id = h.hold_id
WHERE h.hold_status = 'ACTIVE'::v2.appointment_hold_status_enum
  AND a.appointment_id IS NULL
ON CONFLICT (hold_id) WHERE hold_id IS NOT NULL DO NOTHING;

INSERT INTO v2.doctor_occupancy (
    doctor_profile_id, starts_at, ends_at, occupancy_status, appointment_id
)
SELECT h.doctor_profile_id, a.appointment_start, a.appointment_end,
       'ACTIVE'::v2.doctor_occupancy_status_enum, a.appointment_id
FROM v2.appointment a
JOIN v2.appointment_hold h ON h.hold_id = a.source_hold_id
LEFT JOIN v2.doctor_occupancy o ON o.appointment_id = a.appointment_id
WHERE a.appointment_status::text NOT IN ('CANCELLED')
  AND o.occupancy_id IS NULL;

DROP FUNCTION IF EXISTS v2.create_appointment_hold(
    integer, integer, integer, integer, integer, integer, integer, integer,
    v2.booking_type_enum, v2.consultation_type_enum, timestamptz, integer, bigint, boolean
);
CREATE OR REPLACE FUNCTION v2.create_appointment_hold(
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
    p_hold_starts_at timestamptz,
    p_ttl_seconds integer,
    p_duration_seconds bigint,
    p_legacy_compatibility boolean DEFAULT false
) RETURNS TABLE (booking_id varchar(20)) AS $$
DECLARE
    v_booking_id varchar(20);
    v_hold_id uuid;
    v_hold_ends_at timestamptz;
    v_hold_expires_at timestamptz;
    v_projected_doctor_id uuid;
    v_active boolean;
    v_schedule_available boolean;
    v_instant_available boolean;
    v_schedule_config jsonb;
    v_channels text[];
    v_duration_minutes integer;
    v_quote_amount numeric(18, 2);
    v_quote_currency varchar(16);
    v_quote_profile_version bigint;
    v_quote_service_config_version bigint;
    v_timezone text;
    v_start_local timestamp;
    v_end_local timestamp;
    v_periods jsonb;
BEGIN
    IF p_booking_type = 'FollowUp'::v2.booking_type_enum THEN
        RAISE EXCEPTION 'FollowUp is not implemented at the Appointment Hold entry point'
            USING ERRCODE = '22023';
    END IF;
    IF p_duration_seconds <= 0 OR p_ttl_seconds <= 0 THEN
        RAISE EXCEPTION 'Invalid Appointment Hold duration or TTL' USING ERRCODE = '22023';
    END IF;
    IF p_hold_starts_at <= NOW() OR p_hold_starts_at > NOW() + INTERVAL '5 years' THEN
        RAISE EXCEPTION 'Appointment Hold start must be in the allowed future window'
            USING ERRCODE = '22023';
    END IF;

    -- This lock also serializes schedule/config changes when their writers use
    -- this canonical scheduling lock.  The exclusion constraint remains the
    -- final cross-writer invariant for Holds and internal Appointments.
    PERFORM pg_advisory_xact_lock(p_doctor_profile_id::bigint);

    SELECT doctor_id, is_active INTO v_projected_doctor_id, v_active
    FROM v2.doctor_identity
    WHERE doctor_account_id = p_doctor_account_id
      AND doctor_profile_id = p_doctor_profile_id
    FOR UPDATE;
    IF NOT FOUND OR v_active IS NOT TRUE THEN
        RAISE EXCEPTION 'DOCTOR_NOT_AVAILABLE: inactive or unknown doctor' USING ERRCODE = 'P0001';
    END IF;

    SELECT schedule_available, instant_available, schedule_config
    INTO v_schedule_available, v_instant_available, v_schedule_config
    FROM v2.doctor_consultation_config
    WHERE doctor_id = v_projected_doctor_id
    FOR UPDATE;
    IF NOT FOUND
       OR (p_booking_type = 'Schedule'::v2.booking_type_enum AND v_schedule_available IS NOT TRUE)
       OR (p_booking_type = 'Instant'::v2.booking_type_enum AND v_instant_available IS NOT TRUE) THEN
        RAISE EXCEPTION 'DOCTOR_NOT_AVAILABLE: operational availability is disabled' USING ERRCODE = 'P0001';
    END IF;

    -- Missing projection never loosens channel/duration checks, including for
    -- the deprecated legacy function signature.
    SELECT channels, duration_minutes, doctor_fee_amount, doctor_fee_currency,
           profile_version, effective_source_version
    INTO v_channels, v_duration_minutes, v_quote_amount, v_quote_currency,
         v_quote_profile_version, v_quote_service_config_version
    FROM v2.doctor_service_config_projection
    WHERE doctor_id = v_projected_doctor_id
    FOR UPDATE;
    IF NOT FOUND
       OR NOT (p_consultation_channel::text = ANY(v_channels))
        OR p_duration_seconds <> v_duration_minutes::bigint * 60
       OR v_quote_amount IS NULL OR v_quote_currency IS NULL
       OR v_quote_service_config_version IS NULL THEN
        RAISE EXCEPTION 'DOCTOR_NOT_AVAILABLE: unsupported channel or duration' USING ERRCODE = 'P0001';
    END IF;

    v_hold_ends_at := p_hold_starts_at + (p_duration_seconds || ' seconds')::interval;
    v_hold_expires_at := NOW() + (p_ttl_seconds || ' seconds')::interval;

    IF p_booking_type = 'Schedule'::v2.booking_type_enum THEN
        v_timezone := COALESCE(v_schedule_config->>'timezone', 'Asia/Bangkok');
        v_start_local := p_hold_starts_at AT TIME ZONE v_timezone;
        v_end_local := v_hold_ends_at AT TIME ZONE v_timezone;
        IF v_start_local::date <> v_end_local::date THEN
            RAISE EXCEPTION 'DOCTOR_NOT_AVAILABLE: scheduled Hold crosses a schedule day' USING ERRCODE = 'P0001';
        END IF;
        SELECT specific->'periods' INTO v_periods
        FROM jsonb_array_elements(COALESCE(v_schedule_config->'specificDate', '[]'::jsonb)) AS specific
        WHERE specific->>'date' = to_char(v_start_local::date, 'YYYY-MM-DD');
        IF v_periods IS NULL THEN
            v_periods := COALESCE(v_schedule_config->'daysOfWeek'->extract(isodow FROM v_start_local)::text, '[]'::jsonb);
        END IF;
        IF NOT EXISTS (
            SELECT 1 FROM jsonb_array_elements(v_periods) AS period
            WHERE (extract(hour FROM v_start_local)::integer * 60 + extract(minute FROM v_start_local)::integer)
                    >= (period->>'startTime')::integer
              AND (extract(hour FROM v_end_local)::integer * 60 + extract(minute FROM v_end_local)::integer)
                    <= (period->>'endTime')::integer
        ) THEN
            RAISE EXCEPTION 'DOCTOR_NOT_AVAILABLE: outside the current schedule window' USING ERRCODE = 'P0001';
        END IF;
    END IF;

    v_booking_id := v2.generate_booking_id();
    INSERT INTO v2.appointment_hold (
        booking_id, idempotency_key, patient_account_id, patient_profile_id,
        doctor_id, doctor_account_id, doctor_profile_id, biz_unit_id,
        biz_center_id, tenant_id, booking_type, consultation_channel, starts_at,
        ends_at, expires_at, purpose_code, hold_status,
        quoted_amount, quoted_currency, quoted_profile_version,
        quoted_service_config_version
    ) VALUES (
        v_booking_id, 'booking:' || v_booking_id, p_patient_account_id,
        p_patient_profile_id, p_doctor_id, p_doctor_account_id,
        p_doctor_profile_id, p_biz_unit_id, p_biz_center_id, p_tenant_id,
        p_booking_type, p_consultation_channel, p_hold_starts_at, v_hold_ends_at,
        v_hold_expires_at, 'PATIENT_BOOKING', 'ACTIVE'::v2.appointment_hold_status_enum,
        v_quote_amount, upper(v_quote_currency), v_quote_profile_version,
        v_quote_service_config_version
    ) RETURNING hold_id INTO v_hold_id;

    BEGIN
        INSERT INTO v2.doctor_occupancy (
            doctor_profile_id, starts_at, ends_at, occupancy_status, hold_id
        ) VALUES (
            p_doctor_profile_id, p_hold_starts_at, v_hold_ends_at,
            'ACTIVE'::v2.doctor_occupancy_status_enum, v_hold_id
        );
    EXCEPTION WHEN exclusion_violation THEN
        RAISE EXCEPTION 'SLOT_ALREADY_BOOKED: Doctor Occupancy overlaps' USING ERRCODE = '23P01';
    END;

    RETURN QUERY SELECT v_booking_id;
END;
$$ LANGUAGE plpgsql;

-- Follow-ups are booked Appointments, not Holds.  They inherit the parent's
-- already encrypted/intake record by reference and claim the global occupancy
-- ledger atomically; no placeholder PatientPrescreen is manufactured.
CREATE OR REPLACE FUNCTION v2.create_follow_up_appointment(
    p_parent_appointment_id varchar(20), p_appointment_start timestamptz,
    p_consult_duration interval, p_appointment_type v2.appointment_type_enum DEFAULT 'ROUTINE'
) RETURNS jsonb AS $$
DECLARE v_parent v2.appointment%ROWTYPE; v_booking_id varchar(20);
    v_appointment_id varchar(20); v_end timestamptz;
BEGIN
    SELECT * INTO v_parent FROM v2.appointment WHERE appointment_id = p_parent_appointment_id FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'INVALID_FOLLOW_UP: parent appointment not found' USING ERRCODE = '22023'; END IF;
    IF v_parent.appointment_status <> 'FULFILLED'::v2.fhir_appointment_status_enum THEN
        RAISE EXCEPTION 'INVALID_FOLLOW_UP: parent must be fulfilled' USING ERRCODE = '22023';
    END IF;
    IF p_consult_duration <= interval '0' OR p_appointment_start <= NOW() THEN
        RAISE EXCEPTION 'INVALID_FOLLOW_UP: invalid future appointment window' USING ERRCODE = '22023';
    END IF;
    v_end := p_appointment_start + p_consult_duration;
    PERFORM pg_advisory_xact_lock(v_parent.doctor_profile_id::bigint);
    v_booking_id := v2.generate_booking_id(); v_appointment_id := v2.generate_appointment_id();
    INSERT INTO v2.appointment (
      appointment_id, booking_id, prescreen_data_id, source_hold_prescreen_id,
      parent_appointment_id, appointment_status, appointment_start, consult_duration,
      appointment_end, has_follow_up, patient_account_id, patient_profile_id, doctor_id,
      doctor_account_id, doctor_profile_id, biz_unit_id, biz_center_id, tenant_id,
      booking_type, consultation_channel
    ) VALUES (
      v_appointment_id, v_booking_id, v_parent.prescreen_data_id,
      COALESCE(v_parent.source_hold_prescreen_id, v_parent.prescreen_data_id),
      v_parent.appointment_id, 'BOOKED'::v2.fhir_appointment_status_enum,
      p_appointment_start, p_consult_duration, v_end, false, v_parent.patient_account_id,
      v_parent.patient_profile_id, v_parent.doctor_id, v_parent.doctor_account_id,
      v_parent.doctor_profile_id, v_parent.biz_unit_id, v_parent.biz_center_id,
      v_parent.tenant_id, 'FollowUp'::v2.booking_type_enum, v_parent.consultation_channel
    );
    BEGIN
      INSERT INTO v2.doctor_occupancy (doctor_profile_id, starts_at, ends_at, appointment_id)
      VALUES (v_parent.doctor_profile_id, p_appointment_start, v_end, v_appointment_id);
    EXCEPTION WHEN exclusion_violation THEN
      RAISE EXCEPTION 'SLOT_ALREADY_BOOKED: Doctor Occupancy overlaps' USING ERRCODE = '23P01';
    END;
    UPDATE v2.appointment SET has_follow_up = true, modified_at = NOW()
      WHERE appointment_id = v_parent.appointment_id;
    RETURN jsonb_build_object('bookingId', v_booking_id, 'appointmentId', v_appointment_id,
      'appointmentStart', EXTRACT(EPOCH FROM p_appointment_start)::bigint,
      'appointmentEnd', EXTRACT(EPOCH FROM v_end)::bigint, 'doctorId', v_parent.doctor_id,
      'doctorProfileId', v_parent.doctor_profile_id,
      'consultationChannel', v_parent.consultation_channel::text,
      'bizUnitId', v_parent.biz_unit_id, 'bizCenterId', v_parent.biz_center_id,
      'tenantId', v_parent.tenant_id);
END;
$$ LANGUAGE plpgsql;

-- Final reader cutover.  These are the final definitions of active SQL
-- interfaces retained from the reservation era; every one resolves identity
-- through the canonical Appointment/Hold relationship instead.
CREATE OR REPLACE FUNCTION v2.get_appointment_detail(p_booking_id varchar(20))
RETURNS TABLE (
    booking_id varchar(20), appointment_start timestamptz, appointment_end timestamptz,
    appointment_status v2.fhir_appointment_status_enum, booking_type v2.booking_type_enum,
    consultation_channel v2.consultation_type_enum, patient_account_id integer,
    patient_profile_id integer, doctor_account_id integer, doctor_profile_id integer,
    prescreen_data text, prescreen_data_type varchar(255), payment_tx_id bigint,
    payment_tx_ref_id varchar(255)
) AS $$
    SELECT a.booking_id, a.appointment_start, a.appointment_end, a.appointment_status,
      a.booking_type, a.consultation_channel, a.patient_account_id, a.patient_profile_id,
      a.doctor_account_id, a.doctor_profile_id, ps.prescreen_data, ps.prescreen_data_type,
      pay.payment_tx_id, pay.payment_tx_ref_id
    FROM v2.appointment a
    JOIN v2.patient_prescreen ps ON ps.prescreen_id = a.prescreen_data_id
    LEFT JOIN v2.appointment_payment_transaction pay ON pay.appointment_id = a.appointment_id
    WHERE a.booking_id = p_booking_id;
$$ LANGUAGE sql STABLE;

CREATE OR REPLACE FUNCTION v2.list_fulfilled_appointments_by_patient(
    p_account_id integer, p_profile_id integer
) RETURNS TABLE (
    booking_id varchar(20), appointment_start timestamptz, appointment_end timestamptz,
    doctor_account_id integer, doctor_profile_id integer
) AS $$
    SELECT a.booking_id, a.appointment_start, a.appointment_end,
      a.doctor_account_id, a.doctor_profile_id
    FROM v2.appointment a
    WHERE a.patient_account_id = p_account_id
      AND (p_profile_id IS NULL OR a.patient_profile_id = p_profile_id)
      AND a.appointment_status = 'FULFILLED'::v2.fhir_appointment_status_enum
    ORDER BY a.appointment_start DESC LIMIT 50;
$$ LANGUAGE sql STABLE;

CREATE OR REPLACE FUNCTION v2.get_session_details(p_appointment_id varchar(20))
RETURNS TABLE (
    appointment_id varchar(20), booking_id varchar(20), patient_account_id integer,
    patient_profile_id integer, tenant_id integer, doctor_id integer,
    doctor_profile_id integer, session_provider text, session_chat_id text
) AS $$
    SELECT a.appointment_id, a.booking_id, a.patient_account_id, a.patient_profile_id,
      a.tenant_id, a.doctor_id, a.doctor_profile_id,
      COALESCE(si.session_provider, 'TWILIO'), si.session_data->>'sessionChatId'
    FROM v2.appointment a LEFT JOIN v2.session_info si ON si.appointment_id = a.appointment_id
    WHERE a.appointment_id = p_appointment_id;
$$ LANGUAGE sql STABLE;

CREATE OR REPLACE FUNCTION v2.end_active_session(
    p_appointment_id varchar(20), p_doctor_profile_id bigint
) RETURNS bigint AS $$
DECLARE v_rows bigint;
BEGIN
    UPDATE v2.appointment SET appointment_status = 'CONSULTATION_DONE'::v2.fhir_appointment_status_enum,
      modified_at = NOW()
    WHERE appointment_id = p_appointment_id AND doctor_profile_id = p_doctor_profile_id
      AND appointment_status IN ('BOOKED'::v2.fhir_appointment_status_enum,
        'ARRIVED'::v2.fhir_appointment_status_enum);
    GET DIAGNOSTICS v_rows = ROW_COUNT; RETURN v_rows;
END;
$$ LANGUAGE plpgsql;

-- Legacy scheduling readers retain their wire signatures while reading the
-- canonical occupancy ledger.  A Hold and a booked Appointment both consume
-- capacity, so neither may be inferred from v2.reservation after cutover.
CREATE OR REPLACE FUNCTION v2.get_doctor_reservations(
    p_doctor_id integer, p_start_date date, p_end_date date
) RETURNS TABLE (
    booking_id varchar(20), appointment_start timestamptz, appointment_end timestamptz
) AS $$
    SELECT COALESCE(h.booking_id, a.booking_id), o.starts_at, o.ends_at
    FROM v2.doctor_occupancy o
    LEFT JOIN v2.appointment_hold h ON h.hold_id = o.hold_id
    LEFT JOIN v2.appointment a ON a.appointment_id = o.appointment_id
    WHERE COALESCE(h.doctor_id, a.doctor_id) = p_doctor_id
      AND o.occupancy_status = 'ACTIVE'::v2.doctor_occupancy_status_enum
      AND o.starts_at::date >= p_start_date AND o.starts_at::date <= p_end_date
    ORDER BY o.starts_at;
$$ LANGUAGE sql STABLE;

CREATE OR REPLACE FUNCTION v2.get_available_timeslots(
    p_doctor_id integer, p_start_date date, p_end_date date,
    p_working_start_time time DEFAULT '09:00', p_working_end_time time DEFAULT '17:00',
    p_slot_duration_minutes integer DEFAULT 30, p_gap_minutes integer DEFAULT 5
) RETURNS TABLE (
    slot_id bigint, slot_date date, slot_start_time time, slot_end_time time,
    slot_start_timestamptz timestamptz, slot_end_timestamptz timestamptz
) AS $$
    WITH slots AS (
        SELECT row_number() OVER () AS slot_id, d::date AS slot_date,
          t::time AS slot_start_time,
          (t + make_interval(mins => p_slot_duration_minutes))::time AS slot_end_time,
          t::timestamptz AS slot_start_timestamptz,
          (t + make_interval(mins => p_slot_duration_minutes))::timestamptz AS slot_end_timestamptz
        FROM generate_series(p_start_date, p_end_date, interval '1 day') AS d
        CROSS JOIN LATERAL generate_series(
          d::date + p_working_start_time,
          d::date + p_working_end_time - make_interval(mins => p_slot_duration_minutes),
          make_interval(mins => p_slot_duration_minutes + p_gap_minutes)
        ) AS t
    )
    SELECT s.slot_id, s.slot_date, s.slot_start_time, s.slot_end_time,
      s.slot_start_timestamptz, s.slot_end_timestamptz
    FROM slots s
    WHERE NOT EXISTS (
        SELECT 1 FROM v2.doctor_occupancy o
        LEFT JOIN v2.appointment_hold h ON h.hold_id = o.hold_id
        LEFT JOIN v2.appointment a ON a.appointment_id = o.appointment_id
        WHERE COALESCE(h.doctor_id, a.doctor_id) = p_doctor_id
          AND o.occupancy_status = 'ACTIVE'::v2.doctor_occupancy_status_enum
          AND o.starts_at < s.slot_end_timestamptz AND o.ends_at > s.slot_start_timestamptz
    );
$$ LANGUAGE sql STABLE;

-- Remove obsolete callable Reservation APIs before asserting the final runtime
-- surface. The exact 13-argument compatibility wrapper is recreated below and
-- delegates only to canonical Hold creation.
DO $$
DECLARE legacy_function regprocedure;
BEGIN
    FOR legacy_function IN
        SELECT p.oid::regprocedure
        FROM pg_proc p
        JOIN pg_namespace n ON n.oid = p.pronamespace
        WHERE n.nspname = 'v2'
          AND p.proname IN ('get_reservation_by_id', 'create_reservation')
    LOOP
        EXECUTE format('DROP FUNCTION %s CASCADE', legacy_function);
    END LOOP;
END $$;

-- Exact old-pod signature. Deprecated compatibility adapter only: it invokes
-- canonical storage and enforces the same projection/channel/duration rules.
DROP FUNCTION IF EXISTS v2.create_reservation(
    integer, integer, integer, integer, integer, integer, integer, integer,
    v2.booking_type_enum, v2.consultation_type_enum, timestamptz, integer, bigint
);
CREATE OR REPLACE FUNCTION v2.create_reservation(
    p_patient_account_id integer, p_patient_profile_id integer, p_doctor_id integer,
    p_doctor_account_id integer, p_doctor_profile_id integer, p_biz_unit_id integer,
    p_biz_center_id integer, p_tenant_id integer, p_booking_type v2.booking_type_enum,
    p_consultation_channel v2.consultation_type_enum, p_reserved_from timestamptz,
    p_ttl_seconds integer, p_duration_seconds bigint
) RETURNS TABLE (booking_id varchar(20)) AS $$
    SELECT * FROM v2.create_appointment_hold(
        p_patient_account_id, p_patient_profile_id, p_doctor_id,
        p_doctor_account_id, p_doctor_profile_id, p_biz_unit_id, p_biz_center_id,
        p_tenant_id, p_booking_type, p_consultation_channel, p_reserved_from,
        p_ttl_seconds, p_duration_seconds, true
    );
$$ LANGUAGE sql;

-- Patient intake is persisted in the same transaction as Hold creation by
-- the Rust adapter.  Keeping this as a canonical function prevents callers
-- from inventing a sentinel prescreen id and preserves the existing encrypted
-- `patient_prescreen` data/type shape.
DROP FUNCTION IF EXISTS v2.attach_hold_prescreen(varchar, text, varchar);
CREATE OR REPLACE FUNCTION v2.attach_hold_prescreen(
    p_booking_id varchar(20), p_prescreen_data text, p_prescreen_data_type varchar(255)
) RETURNS integer AS $$
DECLARE v_hold v2.appointment_hold%ROWTYPE; v_prescreen_id integer;
BEGIN
    SELECT * INTO v_hold FROM v2.appointment_hold
    WHERE booking_id = p_booking_id FOR UPDATE;
    IF NOT FOUND OR v_hold.hold_status <> 'ACTIVE'::v2.appointment_hold_status_enum THEN
        RAISE EXCEPTION 'INVALID_HOLD_PRESCREEN: active Appointment Hold required' USING ERRCODE = '22023';
    END IF;
    IF v_hold.source_prescreen_id IS NOT NULL THEN
        RETURN v_hold.source_prescreen_id;
    END IF;
    INSERT INTO v2.patient_prescreen (
        booking_id, prescreen_data, prescreen_data_type, user_account_id, user_profile_id
    ) VALUES (
        p_booking_id, p_prescreen_data, p_prescreen_data_type,
        v_hold.patient_account_id, v_hold.patient_profile_id
    ) RETURNING prescreen_id INTO v_prescreen_id;
    UPDATE v2.appointment_hold
    SET source_prescreen_id = v_prescreen_id,
        prescreen_payload = CASE WHEN p_prescreen_data_type = 'RAW_JSON'
            THEN p_prescreen_data::jsonb ELSE prescreen_payload END,
        prescreen_data_type = p_prescreen_data_type,
        modified_at = NOW()
    WHERE hold_id = v_hold.hold_id;
    RETURN v_prescreen_id;
END;
$$ LANGUAGE plpgsql;

DROP FUNCTION IF EXISTS v2.get_booking_state(varchar);
CREATE OR REPLACE FUNCTION v2.get_booking_state(p_booking_id varchar(20))
RETURNS TABLE (
    booking_id varchar(20), patient_account_id integer, patient_profile_id integer,
    tenant_id integer, doctor_id integer, biz_unit_id integer, reservation_status text,
    appointment_status text, reserved_until bigint, appointment_start bigint,
    appointment_end bigint
) AS $$
    SELECT h.booking_id, h.patient_account_id, h.patient_profile_id, h.tenant_id,
           h.doctor_id, COALESCE(h.biz_unit_id, 0),
           CASE h.hold_status::text
             WHEN 'ACTIVE' THEN 'RESERVED'
             WHEN 'EXPIRED' THEN 'RESERVE_EXPIRED'
             WHEN 'RELEASED' THEN CASE WHEN h.release_reason = 'Booked' THEN 'CONFIRMED' ELSE 'CANCELLED' END
             ELSE 'CANCELLED'
           END,
           a.appointment_status::text,
           FLOOR(EXTRACT(EPOCH FROM h.expires_at))::bigint,
           FLOOR(EXTRACT(EPOCH FROM h.starts_at))::bigint,
           FLOOR(EXTRACT(EPOCH FROM h.ends_at))::bigint
    FROM v2.appointment_hold h
    LEFT JOIN v2.appointment a ON a.source_hold_id = h.hold_id
    WHERE h.booking_id = p_booking_id;
$$ LANGUAGE sql STABLE;

DROP FUNCTION IF EXISTS v2.release_appointment_hold(varchar);
CREATE OR REPLACE FUNCTION v2.release_appointment_hold(p_booking_id varchar(20))
RETURNS TABLE (
    booking_id varchar(20), patient_account_id integer, patient_profile_id integer,
    tenant_id integer, doctor_id integer, biz_unit_id integer, reservation_status text,
    appointment_status text, cancelled_at bigint, state_changed boolean
) AS $$
DECLARE v_hold v2.appointment_hold%ROWTYPE; v_appointment_status text;
    v_changed boolean := false; v_expired boolean := false;
BEGIN
    SELECT * INTO v_hold FROM v2.appointment_hold h WHERE h.booking_id = p_booking_id FOR UPDATE;
    IF NOT FOUND THEN RETURN; END IF;
    SELECT a.appointment_status::text INTO v_appointment_status
    FROM v2.appointment a WHERE a.source_hold_id = v_hold.hold_id FOR UPDATE;
    IF v_appointment_status IS NOT NULL AND v_appointment_status NOT IN ('PENDING', 'CANCELLED') THEN
        RAISE EXCEPTION 'Cannot release Appointment Hold for booked appointment' USING ERRCODE = 'P0001';
    END IF;
    IF v_hold.hold_status = 'ACTIVE'::v2.appointment_hold_status_enum THEN
        v_expired := v_hold.expires_at <= NOW();
        UPDATE v2.appointment_hold SET hold_status = CASE WHEN v_expired
            THEN 'EXPIRED'::v2.appointment_hold_status_enum
            ELSE 'RELEASED'::v2.appointment_hold_status_enum END,
          released_at = NOW(), release_reason = CASE WHEN v_expired
            THEN 'HoldExpired' ELSE 'HoldCancelled' END,
          modified_at = NOW() WHERE hold_id = v_hold.hold_id;
        UPDATE v2.doctor_occupancy SET occupancy_status = 'RELEASED', released_at = NOW(),
          release_reason = CASE WHEN v_expired THEN 'HoldExpired' ELSE 'HoldCancelled' END,
          modified_at = NOW()
        WHERE hold_id = v_hold.hold_id AND occupancy_status = 'ACTIVE';
        v_changed := true;
    END IF;
    RETURN QUERY SELECT v_hold.booking_id, v_hold.patient_account_id, v_hold.patient_profile_id,
      v_hold.tenant_id, v_hold.doctor_id, COALESCE(v_hold.biz_unit_id, 0),
      CASE WHEN v_changed AND v_expired THEN 'RESERVE_EXPIRED'
        WHEN v_changed THEN 'CANCELLED' ELSE CASE v_hold.hold_status::text
        WHEN 'EXPIRED' THEN 'RESERVE_EXPIRED' ELSE 'CANCELLED' END END,
      v_appointment_status, FLOOR(EXTRACT(EPOCH FROM NOW()))::bigint, v_changed;
END;
$$ LANGUAGE plpgsql;

DROP FUNCTION IF EXISTS v2.cancel_reserved_booking(varchar);
CREATE OR REPLACE FUNCTION v2.cancel_reserved_booking(p_booking_id varchar(20))
RETURNS TABLE (
    booking_id varchar(20), patient_account_id integer, patient_profile_id integer,
    tenant_id integer, doctor_id integer, biz_unit_id integer, reservation_status text,
    appointment_status text, cancelled_at bigint, state_changed boolean
) AS $$ SELECT * FROM v2.release_appointment_hold(p_booking_id); $$ LANGUAGE sql;

-- Atomically transitions due Holds, releases their occupancy, and enqueues the
-- explicit V1 wire event.  The background worker only invokes this function;
-- it never publishes before the state transaction commits.
DROP FUNCTION IF EXISTS v2.expire_appointment_holds(integer, varchar);
CREATE OR REPLACE FUNCTION v2.expire_appointment_holds(
    p_batch_size integer, p_consultation_topic varchar(255)
) RETURNS TABLE (booking_id varchar(20)) AS $$
BEGIN
    RETURN QUERY
    WITH due AS (
        SELECT h.hold_id FROM v2.appointment_hold h
        WHERE h.hold_status = 'ACTIVE'::v2.appointment_hold_status_enum
          AND h.expires_at <= NOW()
        ORDER BY h.expires_at
        LIMIT GREATEST(p_batch_size, 1)
        FOR UPDATE SKIP LOCKED
    ), expired AS (
        UPDATE v2.appointment_hold h
        SET hold_status = 'EXPIRED', released_at = NOW(), release_reason = 'HoldExpired', modified_at = NOW()
        FROM due WHERE h.hold_id = due.hold_id
        RETURNING h.*
    ), released AS (
        UPDATE v2.doctor_occupancy o
        SET occupancy_status = 'RELEASED', released_at = NOW(), release_reason = 'HoldExpired', modified_at = NOW()
        FROM expired h WHERE o.hold_id = h.hold_id
          AND o.occupancy_status = 'ACTIVE'::v2.doctor_occupancy_status_enum
        RETURNING o.hold_id
    ), enqueued AS (
        INSERT INTO v2.event_outbox (event_id, topic, event_type, aggregate_id, payload, publication_status)
        SELECT v2.generate_uuid_v7(), p_consultation_topic, 'ReservationExpired', h.booking_id,
          jsonb_build_object(
            '__type', 'ReservationExpired', 'bookingId', h.booking_id,
            'patientIdentity', jsonb_build_object('accountId', h.patient_account_id,
              'userProfileId', h.patient_profile_id, 'tenantId', h.tenant_id),
            'doctorId', h.doctor_id, 'bizUnitId', COALESCE(h.biz_unit_id, 0),
            'cancelledAt', FLOOR(EXTRACT(EPOCH FROM h.released_at))::bigint
          ), 'PENDING'
        FROM expired h
        ON CONFLICT DO NOTHING
        RETURNING aggregate_id
    )
    SELECT h.booking_id FROM expired h;
END;
$$ LANGUAGE plpgsql;

-- Global internal-Appointment writer.  It uses the same occupancy ledger and
-- database exclusion invariant as Hold creation.
DROP FUNCTION IF EXISTS v2.create_appointment_internal CASCADE;
CREATE OR REPLACE FUNCTION v2.create_appointment_internal(
    p_patient_account_id integer, p_patient_profile_id integer, p_doctor_account_id integer,
    p_doctor_profile_id integer, p_biz_unit_id integer, p_biz_center_id integer,
    p_tenant_id integer, p_booking_type v2.booking_type_enum,
    p_consultation_channel v2.consultation_type_enum, p_appointment_start timestamptz,
    p_appointment_end timestamptz, p_appointment_status v2.fhir_appointment_status_enum,
    p_payment_tx_id bigint, p_payment_tx_ref_id varchar(255) DEFAULT NULL,
    p_payment_channels jsonb DEFAULT NULL, p_parent_appointment_id varchar(20) DEFAULT NULL,
    p_prescreen_data text DEFAULT '{}', p_prescreen_data_type varchar(255) DEFAULT 'RAW_JSON',
    p_appointment_no varchar(20) DEFAULT NULL
) RETURNS TABLE (booking_id varchar(20), appointment_id varchar(20)) AS $$
DECLARE v_booking_id varchar(20); v_appointment_id varchar(20); v_prescreen_id integer; v_doctor_id integer;
BEGIN
    IF p_appointment_start >= p_appointment_end THEN
        RAISE EXCEPTION 'Invalid Appointment time range' USING ERRCODE = '22023';
    END IF;
    PERFORM pg_advisory_xact_lock(p_doctor_profile_id::bigint);
    SELECT doctor_account_id INTO v_doctor_id FROM v2.doctor_identity
      WHERE doctor_account_id = p_doctor_account_id AND doctor_profile_id = p_doctor_profile_id
        AND is_active IS TRUE FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'DOCTOR_NOT_AVAILABLE: unknown doctor' USING ERRCODE = 'P0001'; END IF;
    v_booking_id := COALESCE(NULLIF(BTRIM(p_appointment_no), ''), v2.generate_booking_id());
    v_appointment_id := v2.generate_appointment_id();
    INSERT INTO v2.patient_prescreen (booking_id, prescreen_data, prescreen_data_type, user_account_id, user_profile_id)
    VALUES (v_booking_id, p_prescreen_data, p_prescreen_data_type, p_patient_account_id, p_patient_profile_id)
    RETURNING prescreen_id INTO v_prescreen_id;
    INSERT INTO v2.appointment (appointment_id, booking_id, prescreen_data_id, parent_appointment_id,
      appointment_status, appointment_start, consult_duration, appointment_end, has_follow_up,
      patient_account_id, patient_profile_id, doctor_id, doctor_account_id, doctor_profile_id,
      biz_unit_id, biz_center_id, tenant_id, booking_type, consultation_channel)
    VALUES (v_appointment_id, v_booking_id, v_prescreen_id, p_parent_appointment_id, p_appointment_status,
      p_appointment_start, p_appointment_end - p_appointment_start, p_appointment_end, false,
      p_patient_account_id, p_patient_profile_id, v_doctor_id, p_doctor_account_id, p_doctor_profile_id,
      p_biz_unit_id, p_biz_center_id, p_tenant_id, p_booking_type, p_consultation_channel);
    INSERT INTO v2.doctor_occupancy (doctor_profile_id, starts_at, ends_at, occupancy_status, appointment_id)
    VALUES (p_doctor_profile_id, p_appointment_start, p_appointment_end,
      'ACTIVE'::v2.doctor_occupancy_status_enum, v_appointment_id);
    INSERT INTO v2.appointment_payment_transaction (appointment_id, payment_tx_id, payment_tx_ref_id, payment_channels)
    VALUES (v_appointment_id, p_payment_tx_id, COALESCE(p_payment_tx_ref_id, v2.generate_uuid_v7()::varchar), p_payment_channels);
    RETURN QUERY SELECT v_booking_id, v_appointment_id;
END;
$$ LANGUAGE plpgsql;

-- The pre-cutover upsert booked without an immutable quote or transactional
-- outbox.  It is deliberately removed: all runtime confirmation must call
-- confirm_payment_and_enqueue_consultation_booked below.
DROP FUNCTION IF EXISTS v2.upsert_payment_transaction(varchar, bigint, varchar, jsonb);

-- Booked-Appointment cancellation owns only Appointment/Occupancy state.  It
-- deliberately does not mutate a Hold, which has already been consumed.
DROP FUNCTION IF EXISTS v2.cancel_appointment(varchar, bigint);
CREATE OR REPLACE FUNCTION v2.cancel_appointment(p_booking_id varchar(20), p_doctor_id bigint)
RETURNS bigint AS $$
DECLARE v_rows bigint;
BEGIN
    UPDATE v2.appointment a SET appointment_status = 'CANCELLED'::v2.fhir_appointment_status_enum,
      modified_at = NOW()
    WHERE a.booking_id = p_booking_id
      AND EXISTS (SELECT 1 FROM v2.doctor_occupancy o
        WHERE o.appointment_id = a.appointment_id AND o.doctor_profile_id = p_doctor_id
          AND o.occupancy_status = 'ACTIVE'::v2.doctor_occupancy_status_enum);
    GET DIAGNOSTICS v_rows = ROW_COUNT;
    IF v_rows > 0 THEN
        UPDATE v2.doctor_occupancy SET occupancy_status = 'RELEASED', released_at = NOW(),
          release_reason = 'AppointmentCancelled', modified_at = NOW()
        WHERE appointment_id IN (SELECT appointment_id FROM v2.appointment WHERE booking_id = p_booking_id)
          AND occupancy_status = 'ACTIVE'::v2.doctor_occupancy_status_enum;
    END IF;
    RETURN v_rows;
END;
$$ LANGUAGE plpgsql;

-- The internal V1 endpoint still calls this historical signature.  Keep its
-- return contract, but route its write through the canonical appointment and
-- occupancy writer; it must never resurrect a v2.reservation row.
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
DECLARE v_booking_id varchar(20); v_appointment_id varchar(20); v_prescreen_id integer;
BEGIN
    SELECT c.booking_id, c.appointment_id INTO v_booking_id, v_appointment_id
    FROM v2.create_appointment_internal(
        p_patient_account_id, p_patient_profile_id, p_doctor_account_id,
        p_doctor_profile_id, p_biz_unit_id, p_biz_center_id, p_tenant_id,
        p_booking_type, p_consultation_channel, p_appointment_start,
        p_appointment_end, 'BOOKED'::v2.fhir_appointment_status_enum, 0,
        NULL, p_payment_channels, p_parent_appointment_id, p_prescreen_data,
        p_prescreen_data_type, NULL
    ) c;
    SELECT a.prescreen_data_id INTO v_prescreen_id
    FROM v2.appointment a WHERE a.appointment_id = v_appointment_id;
    RETURN QUERY SELECT v_booking_id, v_appointment_id,
        'CONFIRMED'::v2.reservation_status_enum,
        'BOOKED'::v2.fhir_appointment_status_enum, v_prescreen_id;
END;
$$ LANGUAGE plpgsql;

-- Summary fulfillment uses Appointment-owned identities.  This final
-- definition preserves the established return contract while removing the
-- last summary-path dependency on legacy reservation rows.
CREATE OR REPLACE FUNCTION v2.create_if_not_existing_summary_note(
    p_appointment_id varchar(20), p_encrypted_data text,
    p_encrypted_data_type varchar(120), p_note_to_staff text,
    p_icd10_codes jsonb, p_prescription_id bigint DEFAULT NULL
) RETURNS jsonb AS $$
DECLARE v_note_id bigint; v_created boolean := true; v_appointment v2.appointment%ROWTYPE;
BEGIN
    SELECT * INTO v_appointment FROM v2.appointment WHERE appointment_id = p_appointment_id FOR UPDATE;
    IF NOT FOUND THEN RAISE EXCEPTION 'INVALID_APPOINTMENT: summary requires Appointment' USING ERRCODE = '22023'; END IF;
    INSERT INTO v2.doctor_summary_note (appointment_id, encrypted_data, encrypted_data_type,
      note_to_staff, icd10_codes, prescription_id)
    VALUES (p_appointment_id, p_encrypted_data, p_encrypted_data_type, p_note_to_staff,
      p_icd10_codes, p_prescription_id)
    ON CONFLICT (appointment_id) DO NOTHING RETURNING summary_note_id INTO v_note_id;
    IF v_note_id IS NULL THEN
      v_created := false;
      SELECT summary_note_id INTO v_note_id FROM v2.doctor_summary_note WHERE appointment_id = p_appointment_id;
    END IF;
    UPDATE v2.appointment SET appointment_status = 'FULFILLED'::v2.fhir_appointment_status_enum,
      modified_at = NOW() WHERE appointment_id = p_appointment_id;
    RETURN jsonb_build_object('created', v_created, 'summaryNoteId', COALESCE(v_note_id, 0),
      'patientAccountId', v_appointment.patient_account_id,
      'userProfileId', v_appointment.patient_profile_id, 'tenantId', v_appointment.tenant_id,
      'bizUnitId', COALESCE(v_appointment.biz_unit_id, 0),
      'bizCenterId', COALESCE(v_appointment.biz_center_id, 0));
END;
$$ LANGUAGE plpgsql;

-- Payment confirmation is a single transaction boundary.  PostgreSQL rolls
-- every preceding write back if the final outbox insert (including a trigger
-- failure) fails.
DROP FUNCTION IF EXISTS v2.confirm_payment_and_enqueue_consultation_booked(
    varchar, bigint, varchar, jsonb, numeric, varchar, bigint, integer, bigint, varchar
);
CREATE OR REPLACE FUNCTION v2.confirm_payment_and_enqueue_consultation_booked(
    p_booking_id varchar(20), p_payment_tx_id bigint, p_payment_tx_ref_id varchar(255),
    p_payment_channels jsonb, p_payment_amount numeric, p_payment_currency varchar(16),
    p_consultation_config_version bigint, p_payment_module_id integer,
    p_booked_at bigint, p_consultation_topic varchar(255)
) RETURNS void AS $$
DECLARE
    v_appointment_id varchar(20); v_hold v2.appointment_hold%ROWTYPE;
    v_existing_payment v2.appointment_payment_transaction%ROWTYPE;
    v_prescreen_id integer; v_active_occupancy_count integer; v_event_id uuid;
BEGIN
    -- Resolve the booked aggregate before inspecting Hold state: this is the
    -- replay path and it remains valid after a Hold is consumed/released.
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
           OR v_existing_payment.consultation_config_version IS DISTINCT FROM p_consultation_config_version
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
           OR v_hold.quoted_service_config_version IS NULL
           OR p_payment_amount <> v_hold.quoted_amount
           OR p_payment_currency <> upper(btrim(p_payment_currency))
           OR p_payment_currency <> v_hold.quoted_currency
           OR p_consultation_config_version <> v_hold.quoted_service_config_version THEN
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
            p_payment_amount, p_payment_currency, p_consultation_config_version,
            p_payment_module_id, p_booked_at
        );
    END IF;

    -- Stable on replay: the first committed row supplies both eventId and
    -- payload; a replay never rewrites either one.
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

-- Legacy reservation interfaces remain only as readers/adapters over canonical
-- aggregates.  Their final definitions contain no legacy table access.
CREATE OR REPLACE FUNCTION v2.get_reservation_by_id(p_reservation_id varchar(20))
RETURNS TABLE (
    booking_id varchar(20), patient_account_id integer, patient_profile_id integer,
    doctor_id integer, doctor_account_id integer, doctor_profile_id integer,
    biz_unit_id integer, biz_center_id integer, tenant_id integer,
    reservation_status v2.reservation_status_enum, reserved_until timestamptz,
    booking_type v2.booking_type_enum, consultation_channel v2.consultation_type_enum,
    appointment_start timestamptz, appointment_end timestamptz
) AS $$
    SELECT h.booking_id, h.patient_account_id, h.patient_profile_id, h.doctor_id,
      h.doctor_account_id, h.doctor_profile_id, h.biz_unit_id, h.biz_center_id, h.tenant_id,
      CASE WHEN h.hold_status = 'ACTIVE'::v2.appointment_hold_status_enum THEN 'RESERVED'::v2.reservation_status_enum
        WHEN h.hold_status = 'EXPIRED'::v2.appointment_hold_status_enum THEN 'RESERVE_EXPIRED'::v2.reservation_status_enum
        WHEN h.hold_status = 'RELEASED'::v2.appointment_hold_status_enum AND h.release_reason = 'Booked' THEN 'CONFIRMED'::v2.reservation_status_enum
        ELSE 'CANCELLED'::v2.reservation_status_enum END,
      h.expires_at, h.booking_type, h.consultation_channel, h.starts_at, h.ends_at
    FROM v2.appointment_hold h WHERE h.booking_id = p_reservation_id;
$$ LANGUAGE sql STABLE;

CREATE OR REPLACE FUNCTION v2.add_patient_verification_tx(
    p_appointment_id varchar(20), p_doctor_profile_id bigint
) RETURNS bigint AS $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM v2.appointment a WHERE a.appointment_id = p_appointment_id
        AND a.doctor_profile_id = p_doctor_profile_id) THEN RETURN 0; END IF;
    INSERT INTO v2.patient_id_verification_transaction (appointment_id) VALUES (p_appointment_id)
      ON CONFLICT (appointment_id) DO NOTHING;
    RETURN 1;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION v2.add_patient_id_verification(
    p_booking_id varchar(20), p_doctor_id bigint
) RETURNS bigint AS $$
DECLARE v_appointment_id varchar(20);
BEGIN
    SELECT appointment_id INTO v_appointment_id FROM v2.appointment WHERE booking_id = p_booking_id;
    IF v_appointment_id IS NULL THEN RETURN 0; END IF;
    RETURN v2.add_patient_verification_tx(v_appointment_id, p_doctor_id);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION v2.get_appointment_chain(p_appointment_id varchar(20)) RETURNS jsonb AS $$
    WITH RECURSIVE chain AS (
        SELECT a.* FROM v2.appointment a WHERE a.appointment_id = p_appointment_id
        UNION ALL
        SELECT child.* FROM v2.appointment child JOIN chain parent
          ON child.parent_appointment_id = parent.appointment_id
    ) SELECT COALESCE(jsonb_agg(jsonb_build_object(
        'appointmentId', appointment_id, 'bookingId', booking_id,
        'parentAppointmentId', parent_appointment_id,
        'appointmentStatus', appointment_status::text,
        'appointmentStart', EXTRACT(EPOCH FROM appointment_start)::bigint,
        'appointmentEnd', EXTRACT(EPOCH FROM appointment_end)::bigint,
        'hasFollowUp', has_follow_up, 'patientAccountId', patient_account_id,
        'doctorId', doctor_id, 'bizUnitId', biz_unit_id, 'bizCenterId', biz_center_id
    ) ORDER BY appointment_start), '[]'::jsonb) FROM chain;
$$ LANGUAGE sql STABLE;

-- Public consultation routes receive bookingId.  Resolve it once at the SQL
-- boundary so no downstream session write can mistake it for appointment_id.
DROP FUNCTION IF EXISTS v2.get_consultation_session(varchar, integer);
CREATE OR REPLACE FUNCTION v2.get_consultation_session(
    p_booking_id varchar(20), p_user_profile_id integer
) RETURNS TABLE (
    appointment_id varchar(20), session_provider_name text, session_data jsonb,
    appointment_status v2.fhir_appointment_status_enum, patient_profile_id bigint,
    doctor_profile_id bigint, consultation_start_time bigint, consultation_end_time bigint,
    consultation_channel v2.consultation_type_enum, payment_channels jsonb,
    is_facial_verified boolean
) AS $$
    SELECT a.appointment_id, COALESCE(si.session_provider, 'TWILIO'), si.session_data,
      a.appointment_status, a.patient_profile_id::bigint, a.doctor_profile_id::bigint,
      EXTRACT(EPOCH FROM a.appointment_start)::bigint, EXTRACT(EPOCH FROM a.appointment_end)::bigint,
      a.consultation_channel, pay.payment_channels,
      CASE WHEN a.consultation_channel = 'video' THEN EXISTS (
        SELECT 1 FROM v2.appointment_facial_upload f WHERE f.appointment_id = a.appointment_id
      ) ELSE true END
    FROM v2.appointment a
    LEFT JOIN v2.session_info si ON si.appointment_id = a.appointment_id
    LEFT JOIN v2.appointment_payment_transaction pay ON pay.appointment_id = a.appointment_id
    WHERE a.booking_id = p_booking_id
      AND (a.patient_profile_id = p_user_profile_id OR a.doctor_profile_id = p_user_profile_id);
$$ LANGUAGE sql STABLE;

CREATE OR REPLACE FUNCTION v2.get_session_details(p_appointment_id varchar(20))
RETURNS TABLE (
    appointment_id varchar(20), booking_id varchar(20), patient_account_id integer,
    patient_profile_id integer, tenant_id integer, doctor_id integer,
    doctor_profile_id integer, session_provider text, session_chat_id text
) AS $$
    SELECT a.appointment_id, a.booking_id, a.patient_account_id, a.patient_profile_id,
      a.tenant_id, a.doctor_id, a.doctor_profile_id,
      COALESCE(si.session_provider, 'TWILIO'), si.session_data->>'sessionChatId'
    FROM v2.appointment a LEFT JOIN v2.session_info si ON si.appointment_id = a.appointment_id
    WHERE a.appointment_id = p_appointment_id;
$$ LANGUAGE sql STABLE;

CREATE OR REPLACE FUNCTION v2.end_active_session(
    p_appointment_id varchar(20), p_doctor_profile_id bigint
) RETURNS bigint AS $$
DECLARE v_rows bigint;
BEGIN
    UPDATE v2.appointment SET appointment_status = 'CONSULTATION_DONE'::v2.fhir_appointment_status_enum,
      modified_at = NOW()
    WHERE appointment_id = p_appointment_id AND doctor_profile_id = p_doctor_profile_id
      AND appointment_status IN ('BOOKED'::v2.fhir_appointment_status_enum,
        'ARRIVED'::v2.fhir_appointment_status_enum);
    GET DIAGNOSTICS v_rows = ROW_COUNT;
    RETURN v_rows;
END;
$$ LANGUAGE plpgsql;

-- Final executable acceptance gate: after every compatibility function has
-- been replaced, no callable runtime function may read the legacy table.
DO $$
DECLARE bad_function text;
BEGIN
    SELECT p.proname || '(' || pg_get_function_identity_arguments(p.oid) || ')'
      INTO bad_function
    FROM pg_proc p JOIN pg_namespace n ON n.oid = p.pronamespace
    WHERE n.nspname = 'v2'
      AND p.prokind = 'f'
      AND pg_get_functiondef(p.oid) ~*
          '(FROM|JOIN|UPDATE|INTO|DELETE[[:space:]]+FROM)[[:space:]]+v2[.]reservation([[:space:](]|$)'
    LIMIT 1;
    IF bad_function IS NOT NULL THEN
        RAISE EXCEPTION 'APMv2 cutover blocked: runtime function still reads v2.reservation: %', bad_function
            USING ERRCODE = 'P1007';
    END IF;
END $$;
