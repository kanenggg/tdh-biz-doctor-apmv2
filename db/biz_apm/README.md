# PostgreSQL Database

This directory contains PostgreSQL database schema, migrations, and seed data for the consultation service.

## Directory Structure

```
postgres/
├── init/                 # Auto-run migrations on container initialization
│   ├── 01_new_biz_apm.sql
│   └── 02_funcs.sql
├── seeds/                # Seed data for testing
│   ├── 001_consultation_test_data.sql
│   ├── run_seeds.sh      # Docker/remote seed runner
│   ├── run_seeds_local.sh # Local development seed runner
│   ├── run_seeds.rs      # Rust seed runner
│   └── README.md
├── migrate/              # Manual migration scripts
│   ├── run_migrations.sh
│   └── run_migrations.rs
├── backup/               # Legacy/backup SQL files
├── 0002_new_biz_apm.sql  # Original schema (deprecated, moved to init/)
├── 0003_funcs.sql        # Original functions (deprecated, moved to init/)
└── README.md
```

## Quick Start

### Using Docker Compose

```bash
# Start PostgreSQL with migrations
docker compose up -d postgres

# Run seeds (after migrations)
docker compose run --rm seed-db

# View logs
docker compose logs -f postgres
```

### Local Development

```bash
# Run migrations manually
psql -h localhost -U biz_apm_admin -d biz_apm -f db/postgres/init/01_new_biz_apm.sql
psql -h localhost -U biz_apm_admin -d biz_apm -f db/postgres/init/02_funcs.sql

# Run seeds
./db/postgres/seeds/run_seeds_local.sh
```

## Database Credentials

- **Host**: localhost (or postgres for Docker)
- **Port**: 5432
- **User**: biz_apm_admin
- **Password**: password
- **Database**: biz_apm

## Tables

### Core Tables
- `booked_slot` - Booking information (patients, doctors, timeslots)
- `appointment` - Appointments linked to bookings
- `session_info` - Consultation session data (Twilio, etc.)
- `patient_in_take_data` - Patient intake data (encrypted)
- `doctor_summary_note` - Doctor's summary notes (encrypted)
- `appointment_payment_tx` - Payment transactions
- `appointment_cancel` - Appointment cancellation records

### Enums
- `booking_type_enum`: INSTANT, SCHEDULE, FOLLOW_UP
- `consultation_type`: VIDEO, VOICE, CHAT
- `appointment_status_enum`: PENDING, CONFIRMED, CONSULTATION_DONE, CANCELLED
- `session_info_status_enum`: EMPTY_ROOM_CREATED, DOCTOR_JOINED, PATIENT_JOINED, ALL_PARTICIPATNS_JOINED, ENDED

## Functions

### `generate_uuid_v7()`
Generates UUID v7 for primary keys.

### `get_consultation_session(p_booking_id, p_user_profile_id)`
Returns consultation session details joining booked_slot, appointment, and session_info.

### `upsert_session_info(p_booking_id, p_session_data)`
Creates or updates session_info for a booking.

## Seed Data

The seeds include:
- 3 test bookings (10001, 10002, 10003)
- 3 test appointments (50001, 50002, 50003)
- 2 test session_info records

See `db/postgres/seeds/README.md` for details.
