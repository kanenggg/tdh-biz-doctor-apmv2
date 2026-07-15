-- sqlfluff:dialect:postgres

ALTER TABLE v2.appointment
ADD COLUMN IF NOT EXISTS appointment_type v2.appointment_type_enum DEFAULT 'ROUTINE';

ALTER TABLE v2.appointment ADD COLUMN IF NOT EXISTS appointment_status_text text;

UPDATE v2.appointment SET appointment_status_text = appointment_status::text;

UPDATE v2.appointment SET appointment_status_text = 'BOOKED' WHERE appointment_status_text = 'CONFIRMED';
UPDATE v2.appointment SET appointment_status_text = 'FULFILLED' WHERE appointment_status_text = 'CONSULTATION_DONE';

ALTER TABLE v2.appointment ALTER COLUMN appointment_status DROP DEFAULT;
ALTER TABLE v2.appointment ALTER COLUMN appointment_status TYPE v2.fhir_appointment_status_enum USING appointment_status_text::v2.fhir_appointment_status_enum;
ALTER TABLE v2.appointment ALTER COLUMN appointment_status SET DEFAULT 'PENDING';

ALTER TABLE v2.appointment DROP COLUMN IF EXISTS appointment_status_text;

DO $$ BEGIN
    DROP TYPE IF EXISTS v2.appointment_status_enum;
EXCEPTION WHEN OTHERS THEN
    RAISE NOTICE 'Could not drop v2.appointment_status_enum: %', SQLERRM;
END $$;
