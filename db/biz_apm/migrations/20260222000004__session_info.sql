-- sqlfluff:dialect:postgres

-- ============================================
-- session_info table
-- ============================================
CREATE TABLE IF NOT EXISTS v2.session_info (
    session_id serial PRIMARY KEY,
    appointment_id varchar(20) NOT NULL UNIQUE,

    session_provider varchar(255) NOT NULL,
    session_status v2.session_info_status_enum DEFAULT 'EMPTY_ROOM_CREATED',
    session_data jsonb,

    created_at timestamptz NOT NULL DEFAULT now(),
    modified_at timestamptz NOT NULL DEFAULT now()
);

-- Foreign key: session_info references appointment
-- ALTER TABLE v2.session_info
-- ADD CONSTRAINT fk_session_info_appointment_id
-- FOREIGN KEY (appointment_id)
-- REFERENCES v2.appointment (appointment_id);

-- Trigger for session_info
CREATE TRIGGER update_session_info_modified_at
BEFORE UPDATE ON v2.session_info
FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();
