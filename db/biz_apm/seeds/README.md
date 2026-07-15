# Database Seeds

This directory contains SQL seed files organized by concern for populating the database with test data.

## Directory Structure

```
seeds/
├── consultation/          # Consultation service seeds
│   └── 001_consultation_test_data.sql
├── appointment/           # Appointment service seeds
│   └── 001_seed_confirmed_appointment.sql
├── doctor_schedule/       # Doctor schedule service seeds
│   └── 002_schedule_config.sql
├── run.sh                # Main runner script
├── run_all.sh            # Shortcut to run all concerns
└── README.md
```

## Running Seeds

### Run All Concerns

```bash
./run.sh
```

### Run Specific Concern

```bash
# Run only consultation seeds
./run.sh consultation

# Run only appointment seeds
./run.sh appointment

# Run only doctor_schedule seeds
./run.sh doctor_schedule
```

### With Environment Variables

```bash
export DB_HOST=localhost
export DB_PORT=5432
export DB_NAME=biz_apm
export DB_USER=biz_apm_admin
export DB_PASSWORD=password

./run.sh doctor_schedule
```

### With Docker Compose

```bash
# Run all concerns
docker compose run --rm seed-db

# Run specific concern
SEED_CONCERN=doctor_schedule docker compose run --rm seed-db
```

## Concerns

### Consultation

Test bookings, appointments, and session info for development.

**Files:**
- `001_consultation_test_data.sql` - Creates test booked_slots, appointments, and session_info

**Test Data:**

| booking_id | patient_profile_id | booking_type | status |
|------------|-------------------|--------------|--------|
| 10001      | 2001              | INSTANT      | Active  |
| 10002      | 2002              | SCHEDULE     | Done    |
| 10003      | 2001              | FOLLOW_UP    | Pending |

### Appointment

Additional appointment test data.

**Files:**
- `001_seed_confirmed_appointment.sql` - Creates a confirmed appointment booking

### Doctor Schedule

Doctor availability schedule configurations.

**Files:**
- `002_schedule_config.sql` - Creates 50 doctors with varied schedule configurations

**Schedule Variants:**
1. 9:00-11:00, 14:00-17:00 (Mon), 10:00-12:00 (Tue)
2. 10:00-12:00, 14:00-16:00, 17:00-18:00 (Mon)
3. 8:00-10:00, 13:00-15:00 (Mon), 16:00-19:00 (Wed)
4. 9:00-13:00 (Tue), 15:00-18:00 (Thu)
5. 11:00-13:00, 15:00-17:00 (Mon), 10:00-12:00, 14:00-16:00 (Fri)

## Adding New Concerns

1. Create a new directory for your concern:
   ```bash
   mkdir -p seeds/new_concern
   ```

2. Add SQL seed files with numbered prefixes (001, 002, etc.):
   ```bash
   touch seeds/new_concern/001_seed_name.sql
   ```

3. Run your concern:
   ```bash
   ./run.sh new_concern
   ```

4. Update this README to document the new concern.
