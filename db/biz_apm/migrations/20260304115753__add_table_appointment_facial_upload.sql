-- sqlfluff:dialect:postgres

-- ============================================
-- appointment_facial_upload table
-- ============================================
CREATE TABLE IF NOT EXISTS v2.appointment_facial_upload (
    appointment_id varchar(20) PRIMARY KEY,
    user_profile_id integer NOT NULL,
    user_account_id integer NOT NULL,
    object_url varchar(250) NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);


-- Indexes for appointment_facial_upload
CREATE INDEX IF NOT EXISTS idx_appointment_facial_upload_user_account_id
ON v2.appointment_facial_upload (user_account_id);

CREATE INDEX IF NOT EXISTS idx_appointment_facial_upload_user_profile_id
ON v2.appointment_facial_upload (user_profile_id);
