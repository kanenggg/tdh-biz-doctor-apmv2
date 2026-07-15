-- sqlfluff:dialect:postgres

-- ADR 0006 follow-up: additive target schema for appointment holds.
-- Runtime code is intentionally not switched in this migration.
CREATE EXTENSION IF NOT EXISTS btree_gist;

DO $$ BEGIN
    CREATE TYPE v2.appointment_hold_status_enum AS ENUM (
        'ACTIVE',
        'RELEASED',
        'EXPIRED',
        'CANCELLED'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE v2.appointment_hold_payment_status_enum AS ENUM (
        'NOT_REQUIRED',
        'PENDING',
        'AUTHORIZED',
        'PAID',
        'FAILED',
        'CANCELLED',
        'REFUNDED'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE v2.appointment_hold_acceptance_status_enum AS ENUM (
        'NOT_REQUIRED',
        'PENDING',
        'ACCEPTED',
        'DECLINED',
        'CANCELLED'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE TABLE IF NOT EXISTS v2.appointment_hold (
    hold_id uuid PRIMARY KEY DEFAULT v2.generate_uuid_v7(),
    idempotency_key varchar(128) NOT NULL UNIQUE,

    patient_account_id integer NOT NULL,
    patient_profile_id integer NOT NULL,
    doctor_id integer NOT NULL,
    doctor_account_id integer NOT NULL,
    doctor_profile_id integer NOT NULL,

    biz_unit_id integer,
    biz_center_id integer,
    tenant_id integer NOT NULL DEFAULT 1,

    booking_type v2.booking_type_enum NOT NULL,
    consultation_channel v2.consultation_type_enum NOT NULL,

    starts_at timestamptz NOT NULL,
    ends_at timestamptz NOT NULL,
    expires_at timestamptz NOT NULL,

    purpose_code varchar(64) NOT NULL,
    purpose_note text,

    hold_status v2.appointment_hold_status_enum NOT NULL DEFAULT 'ACTIVE',

    payment_status v2.appointment_hold_payment_status_enum NOT NULL DEFAULT 'NOT_REQUIRED',
    acceptance_status v2.appointment_hold_acceptance_status_enum NOT NULL DEFAULT 'NOT_REQUIRED',
    payment_required boolean NOT NULL DEFAULT false,
    payment_tx_ref_id varchar(255),
    payment_channels jsonb,

    accepted_at timestamptz,
    accepted_by_account_id integer,
    acceptance_payload jsonb NOT NULL DEFAULT '{}'::jsonb,

    source_prescreen_id integer,
    prescreen_payload jsonb NOT NULL DEFAULT '{}'::jsonb,
    prescreen_data_type varchar(255) NOT NULL DEFAULT 'RAW_JSON',

    released_at timestamptz,
    release_reason text,
    confirmed_appointment_id varchar(20),

    deleted_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    modified_at timestamptz NOT NULL DEFAULT now(),

    CONSTRAINT chk_appointment_hold_time_range CHECK (starts_at < ends_at),
    CONSTRAINT chk_appointment_hold_acceptance_fields CHECK (
        (
            acceptance_status <> 'ACCEPTED'::v2.appointment_hold_acceptance_status_enum
            OR accepted_at IS NOT NULL
        )
        AND (
            accepted_at IS NULL
            OR acceptance_status = 'ACCEPTED'::v2.appointment_hold_acceptance_status_enum
        )
    )
);

CREATE INDEX IF NOT EXISTS idx_appointment_hold_patient_profile_id
ON v2.appointment_hold (patient_profile_id);

CREATE INDEX IF NOT EXISTS idx_appointment_hold_doctor_profile_id
ON v2.appointment_hold (doctor_profile_id);

CREATE INDEX IF NOT EXISTS idx_appointment_hold_status_expires_at
ON v2.appointment_hold (hold_status, expires_at);

CREATE INDEX IF NOT EXISTS idx_appointment_hold_payment_tx_ref_id
ON v2.appointment_hold (payment_tx_ref_id)
WHERE payment_tx_ref_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_appointment_hold_active_range
ON v2.appointment_hold USING gist (
    doctor_profile_id,
    tstzrange(starts_at, ends_at, '[)')
)
WHERE deleted_at IS NULL
  AND hold_status = 'ACTIVE'::v2.appointment_hold_status_enum;

DO $$ BEGIN
    ALTER TABLE v2.appointment_hold
    ADD CONSTRAINT appointment_hold_active_doctor_overlap_excl
    EXCLUDE USING gist (
        doctor_profile_id WITH =,
        tstzrange(starts_at, ends_at, '[)') WITH &&
    )
    WHERE (
        deleted_at IS NULL
        AND hold_status = 'ACTIVE'::v2.appointment_hold_status_enum
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    ALTER TABLE v2.appointment_hold
    ADD CONSTRAINT fk_appointment_hold_source_prescreen_id
    FOREIGN KEY (source_prescreen_id)
    REFERENCES v2.patient_prescreen (prescreen_id);
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TRIGGER update_appointment_hold_modified_at
    BEFORE UPDATE ON v2.appointment_hold
    FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

ALTER TABLE v2.appointment
ADD COLUMN IF NOT EXISTS source_hold_id uuid;

ALTER TABLE v2.appointment
ADD COLUMN IF NOT EXISTS source_hold_prescreen_id integer;

DO $$ BEGIN
    ALTER TABLE v2.appointment
    ADD CONSTRAINT fk_appointment_source_hold_id
    FOREIGN KEY (source_hold_id)
    REFERENCES v2.appointment_hold (hold_id);
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    ALTER TABLE v2.appointment
    ADD CONSTRAINT fk_appointment_source_hold_prescreen_id
    FOREIGN KEY (source_hold_prescreen_id)
    REFERENCES v2.patient_prescreen (prescreen_id);
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

CREATE INDEX IF NOT EXISTS idx_appointment_source_hold_id
ON v2.appointment (source_hold_id)
WHERE source_hold_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_appointment_source_hold_prescreen_id
ON v2.appointment (source_hold_prescreen_id)
WHERE source_hold_prescreen_id IS NOT NULL;
