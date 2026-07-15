-- sqlfluff:dialect:postgres

-- Add a narrow idempotency guard for booking lifecycle events that are
-- expected to occur at most once per booking. The partial predicate avoids
-- blocking legitimate repeated operational/session/config events that share
-- the generic v2.event_outbox table.
--
-- Deploy-order safety: if historical duplicate lifecycle rows already exist,
-- this migration logs a NOTICE and leaves the index absent instead of failing
-- the deploy. Clean duplicates and rerun the CREATE UNIQUE INDEX statement
-- manually if that notice appears.
DO $$
BEGIN
    IF to_regclass('v2.idx_event_outbox_booking_lifecycle_unique') IS NULL THEN
        IF EXISTS (
            SELECT 1
            FROM v2.event_outbox eo
            WHERE eo.aggregate_id IS NOT NULL
              AND eo.event_type IN (
                  'TimeslotReserved',
                  'ReservationCancelled',
                  'ReservationExpired',
                  'ConsultationBooked',
                  'ConsultationCancelled'
              )
            GROUP BY eo.aggregate_id, eo.event_type
            HAVING COUNT(*) > 1
        ) THEN
            RAISE NOTICE 'Skipping idx_event_outbox_booking_lifecycle_unique because duplicate booking lifecycle outbox rows exist';
        ELSE
            EXECUTE '
                CREATE UNIQUE INDEX idx_event_outbox_booking_lifecycle_unique
                ON v2.event_outbox (aggregate_id, event_type)
                WHERE aggregate_id IS NOT NULL
                  AND event_type IN (
                      ''TimeslotReserved'',
                      ''ReservationCancelled'',
                      ''ReservationExpired'',
                      ''ConsultationBooked'',
                      ''ConsultationCancelled''
                  )
            ';
        END IF;
    END IF;
END $$;
