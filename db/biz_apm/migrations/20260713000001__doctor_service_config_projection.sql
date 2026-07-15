-- DoctorApp-owned commercial/service configuration. This is intentionally separate
-- from v2.doctor_consultation_config, which remains APMv2 operational availability.
ALTER TABLE v2.doctor_identity
    ADD COLUMN IF NOT EXISTS profile_version bigint;

CREATE TABLE IF NOT EXISTS v2.doctor_service_config_projection (
    doctor_id uuid PRIMARY KEY REFERENCES v2.doctor_identity (doctor_id),
    channels text[] NOT NULL CHECK (cardinality(channels) > 0 AND channels <@ ARRAY['video', 'voice', 'chat']::text[]),
    languages text[] NOT NULL CHECK (cardinality(languages) > 0),
    duration_minutes integer NOT NULL CHECK (duration_minutes IN (15, 25, 50)),
    doctor_fee_amount numeric(18, 2) NOT NULL CHECK (doctor_fee_amount >= 0),
    doctor_fee_currency text NOT NULL CHECK (length(btrim(doctor_fee_currency)) > 0),
    profile_version bigint NOT NULL CHECK (profile_version > 0),
    source_event_id text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_doctor_service_config_projection_channels
    ON v2.doctor_service_config_projection USING gin (channels);
