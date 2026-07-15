-- sqlfluff:dialect:postgres

-- ============================================
-- appointment_payment_transaction table
-- ============================================
CREATE TABLE IF NOT EXISTS v2.appointment_payment_transaction (
    appointment_id varchar(20) NOT NULL,
    payment_tx_ref_id varchar(255) NOT NULL,
    payment_channels jsonb,

    created_at timestamptz NOT NULL DEFAULT now(),
    cancelled_at timestamptz,
    modified_at timestamptz NOT NULL DEFAULT now()
);

-- Foreign key: appointment_payment_transaction references appointment
-- ALTER TABLE v2.appointment_payment_transaction
-- ADD CONSTRAINT fk_appointment_payment_tx_appointment_id
-- FOREIGN KEY (appointment_id)
-- REFERENCES v2.appointment (appointment_id);

-- Index for appointment_payment_transaction
CREATE INDEX IF NOT EXISTS idx_appointment_payment_tx_payment_tx_ref_id
ON v2.appointment_payment_transaction (payment_tx_ref_id);

CREATE INDEX IF NOT EXISTS idx_appointment_payment_transaction_appointment_id
on v2.appointment_payment_transaction (appointment_id);


-- Trigger for appointment_payment_transaction
CREATE TRIGGER update_appointment_payment_tx_modified_at
BEFORE UPDATE ON v2.appointment_payment_transaction
FOR EACH ROW EXECUTE FUNCTION v2.update_modified_at_column();
