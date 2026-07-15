-- sqlfluff:dialect:postgres

-- Enums
DO $$ BEGIN
    CREATE TYPE v2.booking_type_enum AS ENUM (
        'Instant',
        'Schedule',
        'FollowUp'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE v2.consultation_type_enum AS ENUM (
        'video',
        'voice',
        'chat'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE v2.appointment_status_enum AS ENUM (
        'PENDING',
        'CONFIRMED',
        'CONSULTATION_DONE',
        'CANCELLED'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE v2.reservation_status_enum AS ENUM (
        'RESERVED',
        'CONFIRMED',
        'RESERVE_EXPIRED',
        'CANCELLED'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;

DO $$ BEGIN
    CREATE TYPE v2.session_info_status_enum AS ENUM (
        'EMPTY_ROOM_CREATED',
        'DOCTOR_JOINED',
        'PATIENT_JOINED',
        'ALL_PARTICIPANTS_JOINED',
        'ENDED'
    );
EXCEPTION
    WHEN duplicate_object THEN null;
END $$;
