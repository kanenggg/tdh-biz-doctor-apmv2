# Redis Seed Scripts

This directory contains scripts for seeding Redis with test data.

## Prerequisites

Install the redis Python package:

```bash
pip install redis
```

## Scripts

### init_doctor_timeslot.py

Seeds Redis with doctor schedule configurations for the `doctor-pool` service.

**Usage:**

```bash
python init_doctor_timeslot.py
```

### verify_doctor_timeslot.py

Verifies that doctor schedule data is correctly stored in Redis.

**Usage:**

```bash
python verify_doctor_timeslot.py
```

**What it does:**

- Stores doctor schedule configurations as JSON documents in Redis
- Redis key format: `doctor:{doctor_id}:schedule_config`
- Each config contains:
  - `routine`: Regular weekly schedules (by day of week)
  - `adHoc`: One-off schedule overrides for specific dates

**Sample doctors seeded:**

- `321` - Works Sundays and Mondays with ad-hoc dates
  - Sunday: 09:00-12:00, 14:00-17:00
  - Monday: 08:00-12:00, 13:00-18:00
  - Ad-hoc: 2026-02-12, 2026-02-25
  
- `427` - Works Monday-Friday with lunch breaks
  - Mon-Thu: 09:00-12:00, 13:00-17:00
  - Friday: 09:00-12:00, 13:00-16:00
  - Ad-hoc: 2026-02-20
  
- `abc12345-e89b-12d3-a456-426614174000` - Weekend availability
  - Sunday: 10:00-14:00
  - Saturday: 08:00-12:00

**Example output:**

```
Seeding Redis with doctor schedule configurations...
------------------------------------------------------------
✓ Stored schedule for doctor 321
  Key: doctor:321:schedule_config
  Routine schedules: 2
  Ad-hoc schedules: 2

✓ Stored schedule for doctor 427
  Key: doctor:427:schedule_config
  Routine schedules: 5
  Ad-hoc schedules: 1

✓ Stored schedule for doctor abc12345-e89b-12d3-a456-426614174000
  Key: doctor:abc12345-e89b-12d3-a456-426614174000:schedule_config
  Routine schedules: 2
  Ad-hoc schedules: 0

------------------------------------------------------------
Successfully seeded 3 doctor schedules to Redis
```

**Example API usage after seeding:**

```bash
# Get available timeslots for doctor 321 on Feb 16-17, 2026 (Sunday-Monday)
GET http://localhost:8080/doctors/321/timeslots?startDate=2026-02-16&endDate=2026-02-17&slotDurationMinutes=30
```

## Configuration

By default, scripts connect to:
- Host: `localhost`
- Port: `6379`
- Database: `0`

Modify the connection parameters in the script if your Redis instance uses different settings.

## Data Format

The schedule configuration is stored as JSON at key `doctor:{doctor_id}:schedule_config`:

```json
{
  "routine": [
    {
      "dayOfWeek": 0,
      "times": [
        {
          "startTime": "09:00",
          "endTime": "12:00"
        }
      ]
    }
  ],
  "adHoc": [
    {
      "date": "2026-02-12",
      "times": [
        {
          "startTime": "10:00",
          "endTime": "12:00"
        }
      ]
    }
  ]
}
```

- `routine`: Regular weekly schedules
  - `dayOfWeek`: 0=Sunday, 1=Monday, ..., 6=Saturday
  - `times`: Array of time ranges (HH:MM format)
  
- `adHoc`: One-off schedules for specific dates (overrides routine)
  - `date`: YYYY-MM-DD format
  - `times`: Array of time ranges (HH:MM format)

## Testing

After seeding the data, you can test the API using the HTTP test script:

```bash
../http/test_doctor_timeslots.sh
```

Or manually with curl:

```bash
# Health check
curl http://localhost:8080/health

# Get timeslots
curl "http://localhost:8080/doctors/321/timeslots?startDate=2026-02-16&endDate=2026-02-17&slotDurationMinutes=30"
```
