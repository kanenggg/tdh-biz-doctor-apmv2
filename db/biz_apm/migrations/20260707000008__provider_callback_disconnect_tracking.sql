-- sqlfluff:dialect:postgres

ALTER TABLE v2.session_info
ADD COLUMN IF NOT EXISTS patient_disconnected_at timestamptz,
ADD COLUMN IF NOT EXISTS doctor_disconnected_at timestamptz;

CREATE TABLE IF NOT EXISTS v2.provider_callback_event (
    provider varchar(64) NOT NULL,
    provider_event_id varchar(255) NOT NULL,
    appointment_id varchar(20),
    event_type varchar(128) NOT NULL,
    participant_identity varchar(255),
    payload jsonb NOT NULL,
    processed_at timestamptz NOT NULL DEFAULT now(),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (provider, provider_event_id)
);

CREATE INDEX IF NOT EXISTS idx_provider_callback_event_appointment
ON v2.provider_callback_event (appointment_id, event_type);
