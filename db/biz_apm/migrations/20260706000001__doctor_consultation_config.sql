-- sqlfluff:dialect:postgres

-- Doctor identity projection consumed from DoctorProfileApproved/DoctorProfileDeactivated.
-- APMV2 uses this local projection to resolve canonical doctor_id without
-- synchronous Doctor App calls on config requests.
CREATE TABLE IF NOT EXISTS v2.doctor_identity (
    doctor_id uuid PRIMARY KEY,
    doctor_account_id bigint NOT NULL UNIQUE,
    doctor_profile_id bigint NOT NULL,
    is_active boolean NOT NULL DEFAULT true,
    source_event_id text,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_doctor_identity_account_profile
ON v2.doctor_identity (doctor_account_id, doctor_profile_id);

-- Doctor-facing consultation config source of truth.
-- No biz_unit_id: consultation config is doctor-level. Pool/business routing is projected elsewhere.
CREATE TABLE IF NOT EXISTS v2.doctor_consultation_config (
    doctor_id uuid PRIMARY KEY REFERENCES v2.doctor_identity (doctor_id),
    instant_available boolean NOT NULL DEFAULT false,
    schedule_available boolean NOT NULL DEFAULT false,
    schedule_config jsonb NOT NULL DEFAULT '{"specificDate":[],"daysOfWeek":{},"timezone":"Asia/Bangkok"}'::jsonb,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

