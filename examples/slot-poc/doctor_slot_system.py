"""
Doctor Appointment Slot Availability System
============================================
Dynamic slot generation (no pre-gen in DB)
- Input: Today + 60 days
- Output: Available 30-min slots grouped by Day of Week + Date
- Routine schedule (weekly pattern) + Ad-hoc override (specific date, 1st priority)
- Booked slots excluded
- Redis caching layer

Architecture:
  Schedule Config (DB) --> Slot Generator (Runtime) --> Cache (Redis) --> API Response
                           ^                            |
                           |  Booked Slots (DB) --------+
                           |  invalidate on new booking

Slot alignment: always starts at :00 and :30 (e.g., 09:00, 09:30, 10:00...)
"""

from datetime import datetime, date, time, timedelta
from typing import Optional
from enum import IntEnum
import json
import hashlib

# ============================================================
# 1. MODELS (ORM-agnostic, adapt to Django/SQLAlchemy/etc.)
# ============================================================

class DayOfWeek(IntEnum):
    MONDAY = 0
    TUESDAY = 1
    WEDNESDAY = 2
    THURSDAY = 3
    FRIDAY = 4
    SATURDAY = 5
    SUNDAY = 6


# --- Doctor Routine Schedule (weekly recurring pattern) ---
# Table: doctor_routine_schedules
# Example: Dr.Smith works Mon 09:00-12:00 and 13:00-17:00
#
# | id | doctor_id | day_of_week | start_time | end_time |
# |----|-----------|-------------|------------|----------|
# | 1  | 101       | 0 (Mon)     | 09:00      | 12:00    |
# | 2  | 101       | 0 (Mon)     | 13:00      | 17:00    |
# | 3  | 101       | 2 (Wed)     | 09:00      | 12:00    |

class DoctorRoutineSchedule:
    """Weekly recurring schedule config - multiple time periods per day."""
    def __init__(self, id: int, doctor_id: int, day_of_week: int,
                 start_time: time, end_time: time):
        self.id = id
        self.doctor_id = doctor_id
        self.day_of_week = day_of_week  # 0=Mon..6=Sun
        self.start_time = start_time
        self.end_time = end_time


# --- Doctor Ad-Hoc Schedule (specific date override, HIGHEST PRIORITY) ---
# Table: doctor_adhoc_schedules
# Example: Dr.Smith has special hours on 2026-03-15
#
# | id | doctor_id | specific_date | start_time | end_time |
# |----|-----------|---------------|------------|----------|
# | 1  | 101       | 2026-03-15    | 10:00      | 14:00    |
#
# If ad-hoc exists for a date → use ONLY ad-hoc periods (ignore routine)
# If ad-hoc has ZERO periods for a date → doctor is OFF that day

class DoctorAdhocSchedule:
    """Specific date override - takes priority over routine."""
    def __init__(self, id: int, doctor_id: int, specific_date: date,
                 start_time: time, end_time: time):
        self.id = id
        self.doctor_id = doctor_id
        self.specific_date = specific_date
        self.start_time = start_time
        self.end_time = end_time


# --- Bookings (only actual appointments stored) ---
# Table: bookings
#
# | id | doctor_id | patient_id | slot_start          | slot_end            | status    |
# |----|-----------|------------|---------------------|---------------------|-----------|
# | 1  | 101       | 501        | 2026-03-10 09:00:00 | 2026-03-10 09:30:00 | confirmed |

class Booking:
    """Only actual booked appointments are stored (no empty slots)."""
    def __init__(self, id: int, doctor_id: int, patient_id: int,
                 slot_start: datetime, slot_end: datetime, status: str):
        self.id = id
        self.doctor_id = doctor_id
        self.patient_id = patient_id
        self.slot_start = slot_start
        self.slot_end = slot_end
        self.status = status  # 'confirmed', 'pending', 'cancelled'


# ============================================================
# 2. SQL SCHEMA
# ============================================================

SQL_SCHEMA = """
-- Doctor's weekly recurring schedule (multiple periods per DOW)
CREATE TABLE doctor_routine_schedules (
    id              SERIAL PRIMARY KEY,
    doctor_id       INT NOT NULL,
    day_of_week     SMALLINT NOT NULL CHECK (day_of_week BETWEEN 0 AND 6),
    start_time      TIME NOT NULL,
    end_time        TIME NOT NULL,
    is_active       BOOLEAN DEFAULT TRUE,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),
    
    CHECK (end_time > start_time),
    UNIQUE (doctor_id, day_of_week, start_time)
);

CREATE INDEX idx_routine_doctor_dow ON doctor_routine_schedules (doctor_id, day_of_week)
    WHERE is_active = TRUE;

-- Doctor's ad-hoc schedule override for specific dates (PRIORITY 1)
-- If any row exists for (doctor_id, specific_date) → ONLY use these periods
-- To mark day-off: insert row with is_day_off = TRUE or simply have no time periods
CREATE TABLE doctor_adhoc_schedules (
    id              SERIAL PRIMARY KEY,
    doctor_id       INT NOT NULL,
    specific_date   DATE NOT NULL,
    start_time      TIME,          -- NULL if is_day_off
    end_time        TIME,          -- NULL if is_day_off
    is_day_off      BOOLEAN DEFAULT FALSE,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),
    
    CHECK (
        (is_day_off = TRUE AND start_time IS NULL AND end_time IS NULL)
        OR
        (is_day_off = FALSE AND start_time IS NOT NULL AND end_time IS NOT NULL AND end_time > start_time)
    ),
    UNIQUE (doctor_id, specific_date, start_time)
);

CREATE INDEX idx_adhoc_doctor_date ON doctor_adhoc_schedules (doctor_id, specific_date);

-- Actual bookings only (NO pre-generated empty slots)
CREATE TABLE bookings (
    id              SERIAL PRIMARY KEY,
    doctor_id       INT NOT NULL,
    patient_id      INT NOT NULL,
    slot_start      TIMESTAMPTZ NOT NULL,
    slot_end        TIMESTAMPTZ NOT NULL,
    status          VARCHAR(20) NOT NULL DEFAULT 'pending'
                    CHECK (status IN ('pending', 'confirmed', 'cancelled')),
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),
    
    CHECK (slot_end > slot_start)
);

-- For checking conflicts on booking
CREATE INDEX idx_bookings_doctor_time ON bookings (doctor_id, slot_start, slot_end)
    WHERE status IN ('pending', 'confirmed');

-- Prevent double booking at DB level
CREATE UNIQUE INDEX idx_no_double_booking
    ON bookings (doctor_id, slot_start)
    WHERE status IN ('pending', 'confirmed');
"""


# ============================================================
# 3. REPOSITORY LAYER (Data Access)
# ============================================================

class DoctorScheduleRepository:
    """
    Abstraction over DB queries. Replace internals with your ORM.
    """

    def __init__(self, db_session):
        self.db = db_session

    def get_routine_schedules(self, doctor_id: int) -> list[DoctorRoutineSchedule]:
        """Get all active routine schedule periods for a doctor."""
        # SQL: SELECT * FROM doctor_routine_schedules
        #      WHERE doctor_id = %s AND is_active = TRUE
        #      ORDER BY day_of_week, start_time
        raise NotImplementedError("Wire to your ORM")

    def get_adhoc_schedules(self, doctor_id: int, start_date: date, end_date: date) -> list[DoctorAdhocSchedule]:
        """Get ad-hoc overrides within date range."""
        # SQL: SELECT * FROM doctor_adhoc_schedules
        #      WHERE doctor_id = %s AND specific_date BETWEEN %s AND %s
        #      ORDER BY specific_date, start_time
        raise NotImplementedError("Wire to your ORM")

    def get_adhoc_day_offs(self, doctor_id: int, start_date: date, end_date: date) -> set[date]:
        """Get dates explicitly marked as day-off."""
        # SQL: SELECT DISTINCT specific_date FROM doctor_adhoc_schedules
        #      WHERE doctor_id = %s AND specific_date BETWEEN %s AND %s AND is_day_off = TRUE
        raise NotImplementedError("Wire to your ORM")

    def get_booked_slots(self, doctor_id: int, start_date: date, end_date: date) -> list[Booking]:
        """Get all active bookings in date range."""
        # SQL: SELECT * FROM bookings
        #      WHERE doctor_id = %s
        #        AND slot_start >= %s AND slot_start < %s
        #        AND status IN ('pending', 'confirmed')
        #      ORDER BY slot_start
        raise NotImplementedError("Wire to your ORM")

    def create_booking(self, doctor_id: int, patient_id: int,
                       slot_start: datetime, slot_end: datetime) -> Booking:
        """
        Create booking with DB-level double-booking prevention.
        Uses SELECT FOR UPDATE or unique constraint.
        """
        # SQL (with advisory lock or serializable isolation):
        #
        # BEGIN;
        # -- Check no conflict exists
        # SELECT id FROM bookings
        # WHERE doctor_id = %s
        #   AND status IN ('pending', 'confirmed')
        #   AND slot_start < %s  -- new_end
        #   AND slot_end > %s    -- new_start
        # FOR UPDATE;
        #
        # -- If no rows → safe to insert
        # INSERT INTO bookings (doctor_id, patient_id, slot_start, slot_end, status)
        # VALUES (%s, %s, %s, %s, 'confirmed');
        # COMMIT;
        raise NotImplementedError("Wire to your ORM")


# ============================================================
# 4. CACHE LAYER (Redis)
# ============================================================

class SlotCacheService:
    """
    Redis cache for computed available slots.
    
    Cache Strategy:
    - Key: doctor:{doctor_id}:slots:{date}  (per-date granularity)
    - TTL: 5 minutes (short because bookings change frequently)
    - Invalidation: On new booking / schedule change → delete affected date keys
    
    Why per-date keys?
    - Booking only invalidates 1 date, not entire 60-day window
    - Ad-hoc override only invalidates specific date
    - Fine-grained cache control
    """

    CACHE_TTL_SECONDS = 300  # 5 min
    KEY_PREFIX = "doctor:{doctor_id}:slots:{date}"

    def __init__(self, redis_client):
        self.redis = redis_client

    def _key(self, doctor_id: int, target_date: date) -> str:
        return f"doctor:{doctor_id}:slots:{target_date.isoformat()}"

    def _schedule_version_key(self, doctor_id: int) -> str:
        """Version key to invalidate all cache when schedule config changes."""
        return f"doctor:{doctor_id}:schedule_version"

    def get_slots(self, doctor_id: int, target_date: date) -> Optional[list[dict]]:
        """Get cached slots for a specific date. Returns None if cache miss."""
        key = self._key(doctor_id, target_date)
        data = self.redis.get(key)
        if data:
            return json.loads(data)
        return None

    def set_slots(self, doctor_id: int, target_date: date, slots: list[dict]):
        """Cache computed slots for a specific date."""
        key = self._key(doctor_id, target_date)
        self.redis.setex(key, self.CACHE_TTL_SECONDS, json.dumps(slots))

    def invalidate_date(self, doctor_id: int, target_date: date):
        """Invalidate cache for a specific date (on new booking)."""
        key = self._key(doctor_id, target_date)
        self.redis.delete(key)

    def invalidate_all(self, doctor_id: int):
        """Invalidate all cached slots for doctor (on schedule config change)."""
        pattern = f"doctor:{doctor_id}:slots:*"
        keys = self.redis.keys(pattern)
        if keys:
            self.redis.delete(*keys)


# ============================================================
# 5. CORE: SLOT GENERATOR SERVICE
# ============================================================

SLOT_DURATION_MINUTES = 30

DOW_NAMES = {
    0: "Monday",
    1: "Tuesday",
    2: "Wednesday",
    3: "Thursday",
    4: "Friday",
    5: "Saturday",
    6: "Sunday",
}


class SlotGeneratorService:
    """
    Core engine: Generates available slots on-the-fly.
    
    Algorithm per date:
    1. Determine which time periods apply:
       - If ad-hoc exists for this date → use ONLY ad-hoc periods
       - Else → use routine periods for this DOW
    2. Generate all 30-min aligned slots within each period
    3. Subtract booked slots
    4. Return available slots
    
    Time Complexity per date: O(P + B)
      P = number of periods (typically 1-3)
      B = number of bookings on that date
    
    Total for 60 days: O(60 * (P + B_avg))
    """

    def __init__(self, repository: DoctorScheduleRepository, cache: SlotCacheService):
        self.repo = repository
        self.cache = cache

    def get_available_slots(
        self,
        doctor_id: int,
        from_date: Optional[date] = None,
        days_ahead: int = 60,
    ) -> list[dict]:
        """
        Main API: Get all available slots for next N days.
        
        Returns:
        [
            {
                "date": "2026-03-10",
                "day_of_week": "Monday",
                "day_of_week_number": 0,
                "slots": [
                    {"start": "09:00", "end": "09:30"},
                    {"start": "09:30", "end": "10:00"},
                    ...
                ]
            },
            ...
        ]
        
        Days with zero available slots are excluded from output.
        """
        if from_date is None:
            from_date = date.today()
        
        end_date = from_date + timedelta(days=days_ahead)

        # --- Batch-load all data for the entire range (minimize DB round trips) ---
        routine_schedules = self.repo.get_routine_schedules(doctor_id)
        adhoc_schedules = self.repo.get_adhoc_schedules(doctor_id, from_date, end_date)
        adhoc_day_offs = self.repo.get_adhoc_day_offs(doctor_id, from_date, end_date)
        booked_slots = self.repo.get_booked_slots(doctor_id, from_date, end_date)

        # --- Pre-process into lookup structures ---
        # Routine: { day_of_week: [(start_time, end_time), ...] }
        routine_map: dict[int, list[tuple[time, time]]] = {}
        for s in routine_schedules:
            routine_map.setdefault(s.day_of_week, []).append((s.start_time, s.end_time))

        # Ad-hoc: { date: [(start_time, end_time), ...] }
        adhoc_map: dict[date, list[tuple[time, time]]] = {}
        for s in adhoc_schedules:
            adhoc_map.setdefault(s.specific_date, []).append((s.start_time, s.end_time))

        # Booked: { date: set of (slot_start_time,) }
        booked_map: dict[date, set[time]] = {}
        for b in booked_slots:
            d = b.slot_start.date()
            booked_map.setdefault(d, set()).add(b.slot_start.time())

        # --- Generate slots date by date ---
        result = []
        current = from_date

        while current < end_date:
            # Check cache first
            cached = self.cache.get_slots(doctor_id, current)
            if cached is not None:
                if cached:  # non-empty
                    result.append(cached)
                current += timedelta(days=1)
                continue

            # Determine time periods for this date
            day_slots = self._generate_day_slots(
                target_date=current,
                routine_periods=routine_map.get(current.weekday(), []),
                adhoc_periods=adhoc_map.get(current, None),  # None = no override
                is_adhoc_day_off=current in adhoc_day_offs,
                booked_times=booked_map.get(current, set()),
            )

            # Cache the result (even empty, to avoid re-computation)
            self.cache.set_slots(doctor_id, current, day_slots if day_slots else [])

            if day_slots:
                result.append(day_slots)

            current += timedelta(days=1)

        return result

    def _generate_day_slots(
        self,
        target_date: date,
        routine_periods: list[tuple[time, time]],
        adhoc_periods: Optional[list[tuple[time, time]]],
        is_adhoc_day_off: bool,
        booked_times: set[time],
    ) -> Optional[dict]:
        """
        Generate available slots for a single date.
        
        Priority Logic:
        1. If is_adhoc_day_off → no slots (doctor marked day off)
        2. If adhoc_periods is not None → use ONLY adhoc periods (override routine)
        3. Else → use routine_periods for this DOW
        """
        # Priority 1: Explicit day off
        if is_adhoc_day_off:
            return None

        # Priority 2: Ad-hoc override exists → use it exclusively
        if adhoc_periods is not None:
            periods = adhoc_periods
        else:
            # Priority 3: Fall back to routine
            periods = routine_periods

        if not periods:
            return None

        # Generate 30-min aligned slots from all periods
        available_slots = []
        for period_start, period_end in sorted(periods):
            slots = self._generate_slots_for_period(period_start, period_end, booked_times)
            available_slots.extend(slots)

        if not available_slots:
            return None

        return {
            "date": target_date.isoformat(),
            "day_of_week": DOW_NAMES[target_date.weekday()],
            "day_of_week_number": target_date.weekday(),
            "slots": available_slots,
        }

    @staticmethod
    def _generate_slots_for_period(
        period_start: time,
        period_end: time,
        booked_times: set[time],
    ) -> list[dict]:
        """
        Generate 30-min aligned slots within a single time period.
        
        Alignment: Slots always start at :00 or :30.
        If period_start is 09:15, first slot is 09:30.
        If period_end is 16:45, last slot starts at 16:00 (ends 16:30).
        
        Example:
          period: 09:00 - 12:00
          booked: {09:30, 11:00}
          output: [09:00-09:30, 10:00-10:30, 10:30-11:00, 11:30-12:00]
        """
        slots = []

        # Align start to next :00 or :30
        start_minutes = period_start.hour * 60 + period_start.minute
        if start_minutes % SLOT_DURATION_MINUTES != 0:
            start_minutes = ((start_minutes // SLOT_DURATION_MINUTES) + 1) * SLOT_DURATION_MINUTES

        end_minutes = period_end.hour * 60 + period_end.minute

        current = start_minutes
        while current + SLOT_DURATION_MINUTES <= end_minutes:
            slot_start = time(current // 60, current % 60)
            slot_end_min = current + SLOT_DURATION_MINUTES
            slot_end = time(slot_end_min // 60, slot_end_min % 60)

            # Exclude booked slots
            if slot_start not in booked_times:
                slots.append({
                    "start": slot_start.strftime("%H:%M"),
                    "end": slot_end.strftime("%H:%M"),
                })

            current += SLOT_DURATION_MINUTES

        return slots


# ============================================================
# 6. BOOKING SERVICE (with cache invalidation)
# ============================================================

class BookingService:
    """
    Handles booking creation with:
    - Validation against schedule
    - Double-booking prevention (DB-level)
    - Cache invalidation
    """

    def __init__(self, repository: DoctorScheduleRepository, cache: SlotCacheService,
                 slot_generator: SlotGeneratorService):
        self.repo = repository
        self.cache = cache
        self.slot_generator = slot_generator

    def book_slot(self, doctor_id: int, patient_id: int,
                  slot_start: datetime) -> dict:
        """
        Book a 30-min slot.
        
        Flow:
        1. Validate slot_start alignment (:00 or :30)
        2. Validate slot is within doctor's schedule for that date
        3. Create booking (DB prevents double-booking via unique constraint)
        4. Invalidate cache for that date
        5. Return booking confirmation
        """
        slot_end = slot_start + timedelta(minutes=SLOT_DURATION_MINUTES)

        # 1. Validate alignment
        if slot_start.minute not in (0, 30) or slot_start.second != 0:
            raise ValueError("Slot must start at :00 or :30")

        # 2. Validate within schedule
        target_date = slot_start.date()
        if not self._is_within_schedule(doctor_id, target_date, slot_start.time(), slot_end.time()):
            raise ValueError("Slot is outside doctor's available schedule")

        # 3. Create booking (DB-level conflict check)
        try:
            booking = self.repo.create_booking(
                doctor_id=doctor_id,
                patient_id=patient_id,
                slot_start=slot_start,
                slot_end=slot_end,
            )
        except Exception:
            # Unique constraint violation = slot already booked
            raise ValueError("Slot is already booked")

        # 4. Invalidate cache for this date
        self.cache.invalidate_date(doctor_id, target_date)

        # 5. Return confirmation
        return {
            "booking_id": booking.id,
            "doctor_id": doctor_id,
            "patient_id": patient_id,
            "date": target_date.isoformat(),
            "day_of_week": DOW_NAMES[target_date.weekday()],
            "start": slot_start.time().strftime("%H:%M"),
            "end": slot_end.time().strftime("%H:%M"),
            "status": booking.status,
        }

    def _is_within_schedule(self, doctor_id: int, target_date: date,
                            slot_start: time, slot_end: time) -> bool:
        """Check if a slot falls within doctor's schedule for that date."""
        # Check ad-hoc first (priority)
        adhoc = self.repo.get_adhoc_schedules(doctor_id, target_date, target_date)
        day_offs = self.repo.get_adhoc_day_offs(doctor_id, target_date, target_date)

        if target_date in day_offs:
            return False

        if adhoc:
            periods = [(s.start_time, s.end_time) for s in adhoc]
        else:
            routines = self.repo.get_routine_schedules(doctor_id)
            periods = [
                (s.start_time, s.end_time) for s in routines
                if s.day_of_week == target_date.weekday()
            ]

        return any(
            p_start <= slot_start and slot_end <= p_end
            for p_start, p_end in periods
        )


# ============================================================
# 7. API LAYER (FastAPI example)
# ============================================================

API_EXAMPLE = """
from fastapi import FastAPI, HTTPException, Depends
from datetime import datetime, date

app = FastAPI()

# Dependency injection (wire your actual DB + Redis)
def get_slot_generator() -> SlotGeneratorService:
    ...

def get_booking_service() -> BookingService:
    ...


@app.get("/api/v1/doctors/{doctor_id}/available-slots")
async def get_available_slots(
    doctor_id: int,
    from_date: date = None,  # default: today
    days: int = 60,          # default: 60 days
    generator: SlotGeneratorService = Depends(get_slot_generator),
):
    \"\"\"
    Get available appointment slots for next N days.
    
    Response:
    {
        "doctor_id": 101,
        "range": { "from": "2026-02-13", "to": "2026-04-14" },
        "available_days": [
            {
                "date": "2026-02-16",
                "day_of_week": "Monday",
                "day_of_week_number": 0,
                "slots": [
                    { "start": "09:00", "end": "09:30" },
                    { "start": "09:30", "end": "10:00" },
                    { "start": "10:00", "end": "10:30" },
                    { "start": "13:00", "end": "13:30" },
                    ...
                ]
            },
            {
                "date": "2026-02-17",
                "day_of_week": "Tuesday",
                ...
            }
        ]
    }
    \"\"\"
    slots = generator.get_available_slots(doctor_id, from_date, days)
    from_dt = from_date or date.today()
    return {
        "doctor_id": doctor_id,
        "range": {
            "from": from_dt.isoformat(),
            "to": (from_dt + timedelta(days=days)).isoformat(),
        },
        "available_days": slots,
    }


@app.post("/api/v1/doctors/{doctor_id}/book")
async def book_appointment(
    doctor_id: int,
    patient_id: int,
    slot_start: datetime,  # ISO format: 2026-03-10T09:00:00
    service: BookingService = Depends(get_booking_service),
):
    \"\"\"Book a 30-minute appointment slot.\"\"\"
    try:
        result = service.book_slot(doctor_id, patient_id, slot_start)
        return result
    except ValueError as e:
        raise HTTPException(status_code=409, detail=str(e))


# --- Schedule Admin Endpoints ---

@app.put("/api/v1/doctors/{doctor_id}/schedule/routine")
async def update_routine_schedule(doctor_id: int, ...):
    \"\"\"Update routine schedule → invalidate ALL cache for this doctor.\"\"\"
    # ... update DB ...
    cache.invalidate_all(doctor_id)


@app.put("/api/v1/doctors/{doctor_id}/schedule/adhoc")
async def update_adhoc_schedule(doctor_id: int, specific_date: date, ...):
    \"\"\"Update ad-hoc schedule → invalidate cache for specific date.\"\"\"
    # ... update DB ...
    cache.invalidate_date(doctor_id, specific_date)
"""


# ============================================================
# 8. UNIT TESTS
# ============================================================

def test_slot_generation():
    """Demonstrate the slot generation logic with in-memory data."""
    
    # --- Test: _generate_slots_for_period ---
    
    # Basic: 09:00-12:00, no bookings
    slots = SlotGeneratorService._generate_slots_for_period(
        period_start=time(9, 0),
        period_end=time(12, 0),
        booked_times=set(),
    )
    assert len(slots) == 6  # 09:00, 09:30, 10:00, 10:30, 11:00, 11:30
    assert slots[0] == {"start": "09:00", "end": "09:30"}
    assert slots[-1] == {"start": "11:30", "end": "12:00"}
    print("✅ Basic slot generation: 6 slots from 09:00-12:00")

    # With bookings: 09:30 and 11:00 are booked
    slots = SlotGeneratorService._generate_slots_for_period(
        period_start=time(9, 0),
        period_end=time(12, 0),
        booked_times={time(9, 30), time(11, 0)},
    )
    assert len(slots) == 4  # 09:00, 10:00, 10:30, 11:30
    starts = [s["start"] for s in slots]
    assert "09:30" not in starts
    assert "11:00" not in starts
    print("✅ Booked slots excluded correctly")

    # Alignment: period starts at 09:15 → first slot at 09:30
    slots = SlotGeneratorService._generate_slots_for_period(
        period_start=time(9, 15),
        period_end=time(10, 30),
        booked_times=set(),
    )
    assert len(slots) == 2  # 09:30, 10:00
    assert slots[0] == {"start": "09:30", "end": "10:00"}
    print("✅ Misaligned period_start correctly rounds up to :30")

    # Period too short for any slot
    slots = SlotGeneratorService._generate_slots_for_period(
        period_start=time(9, 0),
        period_end=time(9, 20),
        booked_times=set(),
    )
    assert len(slots) == 0
    print("✅ Period shorter than 30 min produces no slots")

    # --- Test: Priority Logic ---
    
    # Ad-hoc overrides routine
    target = date(2026, 3, 9)   # Monday
    routine_periods = [(time(9, 0), time(17, 0))]  # Full day
    adhoc_periods = [(time(10, 0), time(12, 0))]    # Override: only 10-12

    generator = SlotGeneratorService.__new__(SlotGeneratorService)
    day = generator._generate_day_slots(
        target_date=target,
        routine_periods=routine_periods,
        adhoc_periods=adhoc_periods,   # Ad-hoc EXISTS → use it
        is_adhoc_day_off=False,
        booked_times=set(),
    )
    assert day is not None
    assert len(day["slots"]) == 4  # 10:00, 10:30, 11:00, 11:30
    assert day["day_of_week"] == "Monday"
    print("✅ Ad-hoc override takes priority over routine")

    # Day off
    day = generator._generate_day_slots(
        target_date=target,
        routine_periods=routine_periods,
        adhoc_periods=None,
        is_adhoc_day_off=True,
        booked_times=set(),
    )
    assert day is None
    print("✅ Day-off produces no slots")

    # Routine fallback (no ad-hoc)
    day = generator._generate_day_slots(
        target_date=target,
        routine_periods=routine_periods,
        adhoc_periods=None,   # No ad-hoc → use routine
        is_adhoc_day_off=False,
        booked_times=set(),
    )
    assert day is not None
    assert len(day["slots"]) == 16  # 9:00-17:00 = 16 slots
    print("✅ Routine schedule used when no ad-hoc exists")

    print("\n🎉 All tests passed!")


if __name__ == "__main__":
    test_slot_generation()
