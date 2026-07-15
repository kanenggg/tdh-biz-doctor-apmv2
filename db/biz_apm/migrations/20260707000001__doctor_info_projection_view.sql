-- sqlfluff:dialect:postgres

-- Compatibility projection name for APM doctor onboarding success.
-- The physical projection currently lives in v2.doctor_identity and is populated by
-- consultation-bg-rs doctor_identity consumer. This view exposes the agreed
-- doctor_info_projection store name without duplicating state.
CREATE OR REPLACE VIEW v2.doctor_info_projection AS
SELECT
    doctor_id,
    doctor_account_id,
    doctor_profile_id,
    is_active,
    source_event_id,
    created_at,
    updated_at
FROM v2.doctor_identity;
