-- sqlfluff:dialect:postgres

-- ============================================
-- reservation table
-- ============================================
CREATE TABLE IF NOT EXISTS v2.reservation (
    booking_id varchar(20) PRIMARY KEY,
    patient_account_id integer NOT NULL,
    patient_profile_id integer NOT NULL,

    doctor_id integer NOT NULL,
    doctor_account_id integer NOT NULL,
    doctor_profile_id integer NOT NULL,

    biz_unit_id integer,
    biz_center_id integer,
    tenant_id integer NOT NULL DEFAULT 1,

    reservation_status v2.reservation_status_enum NOT NULL DEFAULT 'RESERVED',
    reserved_until timestamptz NOT NULL,

    booking_type v2.booking_type_enum NOT NULL,
    consultation_channel v2.consultation_type_enum NOT NULL,

    appointment_start timestamptz NOT NULL,
    appointment_end timestamptz NOT NULL,

    cancelled_at timestamptz,
    deleted_at timestamptz,
    created_at timestamptz NOT NULL DEFAULT now(),
    modified_at timestamptz NOT NULL DEFAULT now()
);

-- Indexes for reservation
CREATE INDEX IF NOT EXISTS idx_reservation_patient_account_id
ON v2.reservation (patient_account_id);
CREATE INDEX IF NOT EXISTS idx_reservation_patient_profile_id
ON v2.reservation (patient_profile_id);
CREATE INDEX IF NOT EXISTS idx_reservation_doctor_account_id
ON v2.reservation (doctor_account_id);
CREATE INDEX IF NOT EXISTS idx_reservation_doctor_profile_id
ON v2.reservation (doctor_profile_id);
CREATE INDEX IF NOT EXISTS idx_reservation_appointment_start
ON v2.reservation (appointment_start);
CREATE INDEX IF NOT EXISTS idx_reservation_appointment_end
ON v2.reservation (appointment_end);
CREATE INDEX IF NOT EXISTS idx_reservation_booking_type
ON v2.reservation (booking_type);
CREATE INDEX IF NOT EXISTS idx_reservation_tenant_id
ON v2.reservation (tenant_id);

-- Trigger for reservation
CREATE TRIGGER update_reservation_modified_at
BEFORE UPDATE ON v2.reservation
FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();
