-- no-transaction
-- sqlfluff:dialect:postgres

ALTER TYPE v2.fhir_appointment_status_enum
ADD VALUE IF NOT EXISTS 'CONSULTATION_DONE' AFTER 'FULFILLED';
