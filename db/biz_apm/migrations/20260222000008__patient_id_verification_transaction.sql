-- sqlfluff:dialect:postgres

-- ============================================
-- patient_id_verification_transaction table
-- ============================================
CREATE TABLE IF NOT EXISTS v2.patient_id_verification_transaction (
    appointment_id varchar(
        20
    ) not null constraint patient_id_verification_transaction_pk unique,
    created_at timestamptz NOT NULL DEFAULT now()
);
