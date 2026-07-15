-- sqlfluff:dialect:postgres

CREATE TABLE IF NOT EXISTS doctor_schedule (
    doctor_id UUID PRIMARY KEY,
    schedule_config JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    inactive_at TIMESTAMPTZ
);
