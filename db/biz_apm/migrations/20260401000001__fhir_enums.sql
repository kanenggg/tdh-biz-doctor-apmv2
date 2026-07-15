-- sqlfluff:dialect:postgres

DO $$ BEGIN
    CREATE TYPE v2.fhir_appointment_status_enum AS ENUM (
        'PROPOSED', 'PENDING', 'BOOKED', 'ARRIVED',
        'FULFILLED', 'CANCELLED', 'NOSHOW', 'ENTERED_IN_ERROR'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE v2.appointment_type_enum AS ENUM (
        'ROUTINE', 'WALK_IN', 'EMERGENCY', 'URGENT'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;
