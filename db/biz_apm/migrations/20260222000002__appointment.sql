-- sqlfluff:dialect:postgres

-- ============================================
-- appointment table
-- Note: appointment_id is same as reservation.booking_id (1:1 relationship)
-- ============================================
CREATE TABLE IF NOT EXISTS v2.appointment (
    appointment_id varchar(20) PRIMARY KEY,
    booking_id varchar(20) NOT NULL,
    prescreen_data_id integer NOT NULL,

    parent_appointment_id varchar(20),

    appointment_status v2.appointment_status_enum NOT NULL DEFAULT 'PENDING',

    appointment_start timestamptz NOT NULL,
    consult_duration interval NOT NULL,
    appointment_end timestamptz NOT NULL,

    has_follow_up boolean NOT NULL DEFAULT false,

    created_at timestamptz NOT NULL DEFAULT now(),
    modified_at timestamptz NOT NULL DEFAULT now()
);

-- Foreign key: appointment references reservation
-- ALTER TABLE v2.appointment
-- ADD CONSTRAINT fk_appointment_booking_id
-- FOREIGN KEY (appointment_id)
-- REFERENCES v2.reservation (booking_id);

-- Foreign key: parent appointment (self-referencing)
-- ALTER TABLE v2.appointment
-- ADD CONSTRAINT fk_appointment_parent_appointment_id
-- FOREIGN KEY (parent_appointment_id)
-- REFERENCES v2.appointment (appointment_id);

-- Indexes for appointment
CREATE INDEX IF NOT EXISTS idx_appointment_status
ON v2.appointment (appointment_status);
CREATE INDEX IF NOT EXISTS idx_appointment_parent_appointment_id
ON v2.appointment (parent_appointment_id);

-- Trigger for appointment
CREATE TRIGGER update_appointment_modified_at
BEFORE UPDATE ON v2.appointment
FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();
