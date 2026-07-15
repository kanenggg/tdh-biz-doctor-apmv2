from fastapi import FastAPI, HTTPException, Depends, Query, Path
from datetime import datetime, date, time, timedelta
from pydantic import BaseModel, Field

from doctor_slot_system import (
    SlotGeneratorService,
    BookingService,
    DoctorScheduleRepository,
    SlotCacheService,
)

app = FastAPI(title="Doctor Appointment API", version="1.0.0")


# ============================================================
# Pydantic Models (Request/Response)
# ============================================================

class SlotInfo(BaseModel):
    start: str = Field(..., description="Slot start time in HH:MM format")
    end: str = Field(..., description="Slot end time in HH:MM format")


class AvailableDay(BaseModel):
    date: str = Field(..., description="Date in ISO format (YYYY-MM-DD)")
    day_of_week: str = Field(..., description="Day of week name")
    day_of_week_number: int = Field(..., description="Day of week number (0=Monday, 6=Sunday)")
    slots: list[SlotInfo] = Field(..., description="List of available slots")


class AvailableSlotsResponse(BaseModel):
    doctor_id: int
    range: dict[str, str]
    available_days: list[AvailableDay]


class BookingRequest(BaseModel):
    patient_id: int
    slot_start: datetime = Field(..., description="Slot start datetime in ISO format")


class BookingResponse(BaseModel):
    booking_id: int
    doctor_id: int
    patient_id: int
    date: str
    day_of_week: str
    start: str
    end: str
    status: str


class RoutineScheduleRequest(BaseModel):
    day_of_week: int = Field(..., ge=0, le=6, description="Day of week (0=Monday, 6=Sunday)")
    start_time: str = Field(..., description="Start time in HH:MM format")
    end_time: str = Field(..., description="End time in HH:MM format")


class AdhocScheduleRequest(BaseModel):
    specific_date: date = Field(..., description="Specific date for ad-hoc schedule")
    start_time: Optional[str] = Field(None, description="Start time in HH:MM format (null for day off)")
    end_time: Optional[str] = Field(None, description="End time in HH:MM format (null for day off)")
    is_day_off: bool = Field(False, description="Mark the day as off")


class ErrorResponse(BaseModel):
    detail: str


# ============================================================
# Global Dependencies (TODO: Wire to actual DB + Redis)
# ============================================================

_db_session = None
_redis_client = None

def get_db_session():
    """Get database session (TODO: wire to actual DB)."""
    global _db_session
    return _db_session

def get_redis_client():
    """Get Redis client (TODO: wire to actual Redis)."""
    global _redis_client
    return _redis_client

def get_repository() -> DoctorScheduleRepository:
    """Get schedule repository instance."""
    db = get_db_session()
    if db is None:
        raise HTTPException(status_code=500, detail="Database not configured")
    return DoctorScheduleRepository(db)

def get_cache() -> SlotCacheService:
    """Get cache service instance."""
    redis = get_redis_client()
    if redis is None:
        raise HTTPException(status_code=500, detail="Redis not configured")
    return SlotCacheService(redis)

def get_slot_generator(
    repo: DoctorScheduleRepository = Depends(get_repository),
    cache: SlotCacheService = Depends(get_cache),
) -> SlotGeneratorService:
    """Get slot generator service instance."""
    return SlotGeneratorService(repo, cache)

def get_booking_service(
    repo: DoctorScheduleRepository = Depends(get_repository),
    cache: SlotCacheService = Depends(get_cache),
    generator: SlotGeneratorService = Depends(get_slot_generator),
) -> BookingService:
    """Get booking service instance."""
    return BookingService(repo, cache, generator)


@app.get("/api/v1/doctors/{doctor_id}/available-slots", response_model=AvailableSlotsResponse)
async def get_available_slots(
    doctor_id: int,
    from_date: Optional[date] = Query(None, description="Start date (default: today)"),
    days: int = Query(60, ge=1, le=365, description="Number of days ahead (default: 60, max: 365)"),
    generator: SlotGeneratorService = Depends(get_slot_generator),
):
    """
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
                    ...
                ]
            }
        ]
    }
    """
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


@app.post("/api/v1/doctors/{doctor_id}/book", response_model=BookingResponse)
async def book_appointment(
    doctor_id: int,
    request: BookingRequest,
    service: BookingService = Depends(get_booking_service),
):
    """Book a 30-minute appointment slot."""
    try:
        result = service.book_slot(doctor_id, request.patient_id, request.slot_start)
        return BookingResponse(**result)
    except ValueError as e:
        raise HTTPException(status_code=409, detail=str(e))


# --- Schedule Admin Endpoints ---

@app.put("/api/v1/doctors/{doctor_id}/schedule/routine")
async def update_routine_schedule(
    doctor_id: int,
    request: RoutineScheduleRequest,
    repo: DoctorScheduleRepository = Depends(get_repository),
    cache: SlotCacheService = Depends(get_cache),
):
    """
    Update routine schedule → invalidate ALL cache for this doctor.
    
    This endpoint adds or updates a routine schedule entry for a specific day of week.
    """
    try:
        start_time = time.fromisoformat(request.start_time)
        end_time = time.fromisoformat(request.end_time)
    except ValueError:
        raise HTTPException(status_code=400, detail="Invalid time format, use HH:MM")

    if end_time <= start_time:
        raise HTTPException(status_code=400, detail="End time must be after start time")

    # TODO: Implement DB update
    # Example SQL:
    # INSERT INTO doctor_routine_schedules (doctor_id, day_of_week, start_time, end_time)
    # VALUES (%s, %s, %s, %s)
    # ON CONFLICT (doctor_id, day_of_week, start_time)
    # DO UPDATE SET end_time = EXCLUDED.end_time, updated_at = NOW()
    
    # Invalidate ALL cache for this doctor since routine schedule changed
    cache.invalidate_all(doctor_id)

    return {
        "doctor_id": doctor_id,
        "day_of_week": request.day_of_week,
        "start_time": request.start_time,
        "end_time": request.end_time,
        "message": "Routine schedule updated, cache invalidated",
    }


@app.delete("/api/v1/doctors/{doctor_id}/schedule/routine/{day_of_week}")
async def delete_routine_schedule(
    doctor_id: int,
    day_of_week: int = Path(..., ge=0, le=6, description="Day of week (0=Monday, 6=Sunday)"),
    start_time: Optional[str] = Query(None, description="Start time to delete (if omitted, deletes all for this day)"),
    repo: DoctorScheduleRepository = Depends(get_repository),
    cache: SlotCacheService = Depends(get_cache),
):
    """
    Delete routine schedule entry → invalidate ALL cache for this doctor.
    """
    # TODO: Implement DB delete
    # Example SQL:
    # DELETE FROM doctor_routine_schedules
    # WHERE doctor_id = %s AND day_of_week = %s
    #   AND (start_time = %s OR %s IS NULL)
    
    cache.invalidate_all(doctor_id)

    return {
        "doctor_id": doctor_id,
        "day_of_week": day_of_week,
        "message": "Routine schedule deleted, cache invalidated",
    }


@app.put("/api/v1/doctors/{doctor_id}/schedule/adhoc")
async def update_adhoc_schedule(
    doctor_id: int,
    request: AdhocScheduleRequest,
    repo: DoctorScheduleRepository = Depends(get_repository),
    cache: SlotCacheService = Depends(get_cache),
):
    """
    Update ad-hoc schedule → invalidate cache for specific date.
    
    If is_day_off is true, marks the day as off (no time slots).
    Otherwise, adds or updates time slots for the specific date.
    """
    if request.is_day_off:
        if request.start_time is not None or request.end_time is not None:
            raise HTTPException(
                status_code=400, 
                detail="Cannot specify start_time or end_time when is_day_off is true"
            )

        # TODO: Implement DB insert for day off
        # Example SQL:
        # INSERT INTO doctor_adhoc_schedules (doctor_id, specific_date, is_day_off)
        # VALUES (%s, %s, true)
        # ON CONFLICT (doctor_id, specific_date)
        # DO UPDATE SET is_day_off = true, start_time = NULL, end_time = NULL, updated_at = NOW()
    else:
        if request.start_time is None or request.end_time is None:
            raise HTTPException(
                status_code=400,
                detail="start_time and end_time are required when is_day_off is false"
            )

        try:
            start_time = time.fromisoformat(request.start_time)
            end_time = time.fromisoformat(request.end_time)
        except ValueError:
            raise HTTPException(status_code=400, detail="Invalid time format, use HH:MM")

        if end_time <= start_time:
            raise HTTPException(status_code=400, detail="End time must be after start time")

        # TODO: Implement DB insert for time slots
        # Example SQL:
        # INSERT INTO doctor_adhoc_schedules (doctor_id, specific_date, start_time, end_time, is_day_off)
        # VALUES (%s, %s, %s, %s, false)
        # ON CONFLICT (doctor_id, specific_date, start_time)
        # DO UPDATE SET end_time = EXCLUDED.end_time, updated_at = NOW()

    # Invalidate cache for this specific date only
    cache.invalidate_date(doctor_id, request.specific_date)

    return {
        "doctor_id": doctor_id,
        "date": request.specific_date.isoformat(),
        "is_day_off": request.is_day_off,
        "message": "Ad-hoc schedule updated, cache invalidated for date",
    }


@app.delete("/api/v1/doctors/{doctor_id}/schedule/adhoc/{specific_date}")
async def delete_adhoc_schedule(
    doctor_id: int,
    specific_date: date,
    repo: DoctorScheduleRepository = Depends(get_repository),
    cache: SlotCacheService = Depends(get_cache),
):
    """
    Delete ad-hoc schedule for a specific date → invalidate cache for that date.
    """
    # TODO: Implement DB delete
    # Example SQL:
    # DELETE FROM doctor_adhoc_schedules
    # WHERE doctor_id = %s AND specific_date = %s
    
    cache.invalidate_date(doctor_id, specific_date)

    return {
        "doctor_id": doctor_id,
        "date": specific_date.isoformat(),
        "message": "Ad-hoc schedule deleted, cache invalidated for date",
    }


# ============================================================
# Health Check Endpoint
# ============================================================

@app.get("/health")
async def health_check():
    """Health check endpoint."""
    return {"status": "healthy", "service": "doctor-appointment-api"}


# ============================================================
# Run Server
# ============================================================

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
