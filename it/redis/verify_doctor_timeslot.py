from __future__ import annotations

import json
import redis


def verify_redis_data():
    """Verify that doctor schedule data is correctly stored in Redis"""
    r = redis.Redis(host="localhost", port=6379, db=0, decode_responses=True)
    
    print("Verifying doctor schedule configurations in Redis...")
    print("=" * 70)
    
    # Get all doctor schedule keys
    keys = r.keys("doctor:*:schedule_config")
    
    if not keys:
        print("❌ No doctor schedules found in Redis!")
        print("   Run init_doctor_timeslot.py first to seed the data.")
        return
    
    print(f"Found {len(keys)} doctor schedule(s)\n")
    
    for key in sorted(keys):
        doctor_id = key.split(":")[1]
        schedule_json = r.get(key)
        
        if not schedule_json:
            print(f"❌ Doctor {doctor_id}: No data found")
            continue
        
        try:
            schedule = json.loads(schedule_json)
            
            print(f"✓ Doctor {doctor_id}")
            print(f"  Key: {key}")
            print(f"  Routine schedules: {len(schedule.get('routine', []))}")
            
            for routine in schedule.get('routine', []):
                day_names = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']
                day_of_week = routine.get('dayOfWeek', -1)
                day_name = day_names[day_of_week] if 0 <= day_of_week <= 6 else 'Unknown'
                print(f"    - {day_name} ({day_of_week}): {len(routine.get('times', []))} time slots")
                for time_range in routine.get('times', []):
                    print(f"      {time_range.get('startTime')} - {time_range.get('endTime')}")
            
            print(f"  Ad-hoc schedules: {len(schedule.get('adHoc', []))}")
            for adhoc in schedule.get('adHoc', []):
                print(f"    - {adhoc.get('date')}: {len(adhoc.get('times', []))} time slots")
                for time_range in adhoc.get('times', []):
                    print(f"      {time_range.get('startTime')} - {time_range.get('endTime')}")
            
            print()
            
        except json.JSONDecodeError as e:
            print(f"❌ Doctor {doctor_id}: Invalid JSON - {e}")
            print(f"   Raw data: {schedule_json[:100]}...")
            print()
    
    print("=" * 70)
    print("Verification complete!")
    print()
    print("Example API requests:")
    print("  GET http://localhost:8080/doctors/321/timeslots?startDate=2026-02-16&endDate=2026-02-17&slotDurationMinutes=30")
    print("  GET http://localhost:8080/doctors/427/timeslots?startDate=2026-02-17&endDate=2026-02-21&slotDurationMinutes=60")


if __name__ == "__main__":
    verify_redis_data()
