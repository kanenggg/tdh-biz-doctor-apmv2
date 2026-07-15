-- sqlfluff:dialect:postgres

-- ============================================
-- Appointment Detail (read endpoint support)
-- 1. Add payment_tx_id column to appointment_payment_transaction
-- 2. Create v2.get_appointment_detail function
-- ============================================

-- ----- 1. payment_tx_id column -----

ALTER TABLE v2.appointment_payment_transaction
    ADD COLUMN IF NOT EXISTS payment_tx_id bigint NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_appointment_payment_tx_payment_tx_id
    ON v2.appointment_payment_transaction (payment_tx_id);

-- NOTE: The DEFAULT 0 is a temporary safety net so the existing
-- create_appointment_internal / create_confirmed_appointment functions
-- continue to work without modification in this slice. Slice 5 updates
-- those functions to pass an explicit payment_tx_id from the payment
-- transaction event, after which the default could be dropped (left in
-- place for now to avoid breaking pre-Slice-5 callers).

-- ----- 2. v2.get_appointment_detail -----

DROP FUNCTION IF EXISTS v2.get_appointment_detail(varchar);

CREATE OR REPLACE FUNCTION v2.get_appointment_detail(p_booking_id varchar(20))
RETURNS TABLE (
    booking_id varchar(20),
    appointment_start timestamptz,
    appointment_end timestamptz,
    appointment_status v2.fhir_appointment_status_enum,
    booking_type v2.booking_type_enum,
    consultation_channel v2.consultation_type_enum,
    patient_account_id integer,
    patient_profile_id integer,
    doctor_account_id integer,
    doctor_profile_id integer,
    prescreen_data text,
    prescreen_data_type varchar(255),
    payment_tx_id bigint,
    payment_tx_ref_id varchar(255)
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        r.booking_id,
        r.appointment_start,
        r.appointment_end,
        a.appointment_status,
        r.booking_type,
        r.consultation_channel,
        r.patient_account_id,
        r.patient_profile_id,
        r.doctor_account_id,
        r.doctor_profile_id,
        ps.prescreen_data,
        ps.prescreen_data_type,
        pay.payment_tx_id,
        pay.payment_tx_ref_id
    FROM v2.reservation r
    INNER JOIN v2.appointment a ON a.appointment_id = r.booking_id
    INNER JOIN v2.patient_prescreen ps ON ps.booking_id = r.booking_id
    INNER JOIN v2.appointment_payment_transaction pay ON pay.appointment_id = r.booking_id
    WHERE r.booking_id = p_booking_id;
END;
$$ LANGUAGE plpgsql;
