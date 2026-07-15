from __future__ import annotations

import json
import redis
from typing import TypedDict


class TimeRange(TypedDict):
    startTime: str
    endTime: str


class RoutineSchedule(TypedDict):
    dayOfWeek: int
    times: list[TimeRange]


class AdHocSchedule(TypedDict):
    date: str
    times: list[TimeRange]


class DoctorScheduleConfig(TypedDict):
    routine: list[RoutineSchedule]
    adHoc: list[AdHocSchedule]


# Sample doctor schedule configurations
# These will be stored in Redis as JSON at key: doctor:{doctor_id}:schedule_config
doctor_schedules: dict[str, DoctorScheduleConfig] = {
    # Doctor 321: Works on Sundays (0) and Mondays (1)
    "321": {
        "routine": [
            {
                "dayOfWeek": 0,  # Sunday
                "times": [
                    {"startTime": "09:00", "endTime": "12:00"},
                    {"startTime": "14:00", "endTime": "17:00"},
                ],
            },
            {
                "dayOfWeek": 1,  # Monday
                "times": [
                    {"startTime": "08:00", "endTime": "12:00"},
                    {"startTime": "13:00", "endTime": "18:00"},
                ],
            },
        ],
        "adHoc": [
            {
                "date": "2026-02-12",
                "times": [
                    {"startTime": "10:00", "endTime": "12:00"},
                    {"startTime": "14:00", "endTime": "16:00"},
                ],
            },
            {
                "date": "2026-02-25",
                "times": [
                    {"startTime": "09:00", "endTime": "15:00"},
                ],
            },
        ],
    },
    # Doctor 427: Works weekdays with lunch break
    "427": {
        "routine": [
            {
                "dayOfWeek": 1,  # Monday
                "times": [
                    {"startTime": "09:00", "endTime": "12:00"},
                    {"startTime": "13:00", "endTime": "17:00"},
                ],
            },
            {
                "dayOfWeek": 2,  # Tuesday
                "times": [
                    {"startTime": "09:00", "endTime": "12:00"},
                    {"startTime": "13:00", "endTime": "17:00"},
                ],
            },
            {
                "dayOfWeek": 3,  # Wednesday
                "times": [
                    {"startTime": "09:00", "endTime": "12:00"},
                    {"startTime": "13:00", "endTime": "17:00"},
                ],
            },
            {
                "dayOfWeek": 4,  # Thursday
                "times": [
                    {"startTime": "09:00", "endTime": "12:00"},
                    {"startTime": "13:00", "endTime": "17:00"},
                ],
            },
            {
                "dayOfWeek": 5,  # Friday
                "times": [
                    {"startTime": "09:00", "endTime": "12:00"},
                    {"startTime": "13:00", "endTime": "16:00"},
                ],
            },
        ],
        "adHoc": [
            {
                "date": "2026-02-20",
                "times": [
                    {"startTime": "10:00", "endTime": "14:00"},
                ],
            },
        ],
    },
    # Doctor abc123: Full UUID example with weekend availability
    "abc12345-e89b-12d3-a456-426614174000": {
        "routine": [
            {
                "dayOfWeek": 0,  # Sunday
                "times": [
                    {"startTime": "10:00", "endTime": "14:00"},
                ],
            },
            {
                "dayOfWeek": 6,  # Saturday
                "times": [
                    {"startTime": "08:00", "endTime": "12:00"},
                ],
            },
        ],
        "adHoc": [],
    },
}


if __name__ == "__main__":
    r = redis.Redis(host="localhost", port=6379, db=0, decode_responses=True)
    
    print("Seeding Redis with doctor schedule configurations...")
    print("-" * 60)
    
    for doctor_id, schedule_config in doctor_schedules.items():
        # Store schedule config as JSON
        redis_key = f"doctor:{doctor_id}:schedule_config"
        schedule_json = json.dumps(schedule_config)
        
        r.set(redis_key, schedule_json)
        print(f"✓ Stored schedule for doctor {doctor_id}")
        print(f"  Key: {redis_key}")
        print(f"  Routine schedules: {len(schedule_config['routine'])}")
        print(f"  Ad-hoc schedules: {len(schedule_config['adHoc'])}")
        print()
    
    print("-" * 60)
    print(f"Successfully seeded {len(doctor_schedules)} doctor schedules to Redis")
    print()
    print("Example query:")
    print("  GET /doctors/321/timeslots?startDate=2026-02-16&endDate=2026-02-17&slotDurationMinutes=30")
