-- sqlfluff:dialect:postgres

-- ============================================
-- Prepare script for FHIR appointment status migration
-- Run this BEFORE the migration scripts (001-005)
-- Idempotent: safe to run multiple times
-- ============================================

-- 1. Report current appointment_status distribution
DO $$ DECLARE
    rec RECORD;
BEGIN
    RAISE NOTICE '===== PRE-MIGRATION DATA AUDIT =====';

    RAISE NOTICE 'Current appointment_status distribution:';
    FOR rec IN
        SELECT appointment_status::text AS status, COUNT(*) AS cnt
        FROM v2.appointment
        GROUP BY appointment_status
        ORDER BY appointment_status::text
    LOOP
        RAISE NOTICE '  %: % rows', rec.status, rec.cnt;
    END LOOP;

    RAISE NOTICE 'Total appointments: %', (SELECT COUNT(*) FROM v2.appointment);
    RAISE NOTICE 'Appointments with parent (follow-ups): %', (SELECT COUNT(*) FROM v2.appointment WHERE parent_appointment_id IS NOT NULL);
    RAISE NOTICE 'Appointments with has_follow_up = true: %', (SELECT COUNT(*) FROM v2.appointment WHERE has_follow_up = true);
    RAISE NOTICE '=====================================';
END $$;

-- 2. Validate that all existing status values have a mapping to FHIR
-- Old values: PENDING, CONFIRMED, CONSULTATION_DONE, CANCELLED
-- FHIR mapping: PENDING -> PENDING, CONFIRMED -> BOOKED, CONSULTATION_DONE -> FULFILLED, CANCELLED -> CANCELLED
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM v2.appointment
        WHERE appointment_status::text NOT IN ('PENDING', 'CONFIRMED', 'CONSULTATION_DONE', 'CANCELLED')
    ) THEN
        RAISE EXCEPTION 'Found unexpected appointment_status values that have no FHIR mapping. '
            'Run: SELECT DISTINCT appointment_status::text FROM v2.appointment;';
    ELSE
        RAISE NOTICE 'All existing appointment_status values have valid FHIR mappings.';
    END IF;
END $$;

-- 3. Check that appointment table has expected columns for migration
DO $$ DECLARE
    v_col_count integer;
BEGIN
    SELECT COUNT(*) INTO v_col_count
    FROM information_schema.columns
    WHERE table_schema = 'v2'
      AND table_name = 'appointment'
      AND column_name IN ('appointment_id', 'appointment_status', 'parent_appointment_id', 'has_follow_up');

    IF v_col_count != 4 THEN
        RAISE EXCEPTION 'Expected 4 key columns in v2.appointment, found %. Schema may have changed.', v_col_count;
    ELSE
        RAISE NOTICE 'Appointment table schema validation passed.';
    END IF;
END $$;

-- 4. Check no orphan appointments (no matching reservation)
DO $$ DECLARE
    rec RECORD;
BEGIN
    IF EXISTS (
        SELECT a.appointment_id
        FROM v2.appointment a
        LEFT JOIN v2.reservation r ON r.booking_id = a.appointment_id
        WHERE r.booking_id IS NULL
    ) THEN
        RAISE WARNING 'Found orphan appointments without matching reservations. These may cause issues during migration.';
        FOR rec IN
            SELECT a.appointment_id
            FROM v2.appointment a
            LEFT JOIN v2.reservation r ON r.booking_id = a.appointment_id
            WHERE r.booking_id IS NULL
        LOOP
            RAISE WARNING '  Orphan: %', rec.appointment_id;
        END LOOP;
    ELSE
        RAISE NOTICE 'No orphan appointments found.';
    END IF;
END $$;

-- 5. Check that appointment_type_enum does not already exist
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_type t
        JOIN pg_namespace n ON n.oid = t.typnamespace
        WHERE n.nspname = 'v2' AND t.typname = 'appointment_type_enum'
    ) THEN
        RAISE WARNING 'v2.appointment_type_enum already exists. Migration 001 may have been partially run.';
    ELSE
        RAISE NOTICE 'v2.appointment_type_enum does not exist yet. Safe to proceed.';
    END IF;
END $$;

-- 6. Check that fhir_appointment_status_enum does not already exist
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_type t
        JOIN pg_namespace n ON n.oid = t.typnamespace
        WHERE n.nspname = 'v2' AND t.typname = 'fhir_appointment_status_enum'
    ) THEN
        RAISE WARNING 'v2.fhir_appointment_status_enum already exists. Migration 001 may have been partially run.';
    ELSE
        RAISE NOTICE 'v2.fhir_appointment_status_enum does not exist yet. Safe to proceed.';
    END IF;
END $$;

-- 7. Check active connections / locks on appointment table
DO $$ BEGIN
    IF EXISTS (
        SELECT 1 FROM pg_locks l
        JOIN pg_class c ON c.oid = l.relation
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'v2' AND c.relname = 'appointment'
          AND l.mode IN ('AccessExclusiveLock', 'RowExclusiveLock')
          AND NOT l.granted
    ) THEN
        RAISE WARNING 'There are pending locks on v2.appointment. Consider running migration during low traffic.';
    ELSE
        RAISE NOTICE 'No problematic locks on v2.appointment.';
    END IF;
END $$;

DO $$ BEGIN
    RAISE NOTICE '===== PREPARATION CHECK COMPLETE =====';
    RAISE NOTICE 'If no EXCEPTIONs were raised, it is safe to run migrations 001-005.';
    RAISE NOTICE '=======================================';
END $$;
