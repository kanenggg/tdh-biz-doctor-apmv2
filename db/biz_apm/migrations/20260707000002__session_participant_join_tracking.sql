-- sqlfluff:dialect:postgres

-- Track first participant join times so session lifecycle events can be emitted
-- idempotently when clients request session-info / access tokens.
ALTER TABLE v2.session_info
ADD COLUMN IF NOT EXISTS patient_joined_at timestamptz,
ADD COLUMN IF NOT EXISTS doctor_joined_at timestamptz;

CREATE INDEX IF NOT EXISTS idx_session_info_patient_joined_at
ON v2.session_info (patient_joined_at);

CREATE INDEX IF NOT EXISTS idx_session_info_doctor_joined_at
ON v2.session_info (doctor_joined_at);
