-- The committed DoctorApp approval event has no profileVersion. Its durable
-- ordering coordinate is occurredAt, stored separately from a future version.
ALTER TABLE v2.doctor_identity
    ADD COLUMN IF NOT EXISTS source_occurred_at bigint;

ALTER TABLE v2.doctor_service_config_projection
    ALTER COLUMN profile_version DROP NOT NULL;

ALTER TABLE v2.doctor_service_config_projection
    ADD COLUMN IF NOT EXISTS source_occurred_at bigint;

ALTER TABLE v2.doctor_service_config_projection
    ADD COLUMN IF NOT EXISTS effective_source_version bigint;

UPDATE v2.doctor_service_config_projection
SET effective_source_version = COALESCE(profile_version, source_occurred_at)
WHERE effective_source_version IS NULL;

CREATE OR REPLACE FUNCTION v2.set_doctor_service_config_effective_source_version()
RETURNS trigger AS $$
BEGIN
    NEW.effective_source_version := COALESCE(NEW.profile_version, NEW.source_occurred_at);
    IF NEW.effective_source_version IS NULL THEN
        RAISE EXCEPTION 'doctor service config requires profileVersion or occurredAt'
            USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_doctor_service_config_effective_source_version
    ON v2.doctor_service_config_projection;
CREATE TRIGGER trg_doctor_service_config_effective_source_version
BEFORE INSERT OR UPDATE OF profile_version, source_occurred_at
ON v2.doctor_service_config_projection
FOR EACH ROW EXECUTE FUNCTION v2.set_doctor_service_config_effective_source_version();

ALTER TABLE v2.doctor_service_config_projection
    ALTER COLUMN effective_source_version SET NOT NULL;

COMMENT ON COLUMN v2.doctor_identity.profile_version IS
    'Optional producer profileVersion. Null means the committed DoctorApp origin/main contract was consumed.';
COMMENT ON COLUMN v2.doctor_identity.source_occurred_at IS
    'Producer occurredAt used for ordering unversioned DoctorProfile events; never synthesized.';
COMMENT ON COLUMN v2.doctor_service_config_projection.profile_version IS
    'Optional producer profileVersion; it is not fabricated for committed unversioned events.';
COMMENT ON COLUMN v2.doctor_service_config_projection.source_occurred_at IS
    'Producer occurredAt copied from the event and used as the source ordering coordinate when profileVersion is null.';
COMMENT ON COLUMN v2.doctor_service_config_projection.effective_source_version IS
    'Explicit immutable quote coordinate: profileVersion when supplied, otherwise source_occurred_at. This is not a fabricated profileVersion.';
