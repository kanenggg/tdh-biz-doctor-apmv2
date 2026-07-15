#!/bin/bash

# Test script for doctor-pool timeslot API
# Prerequisites:
#   1. Redis running with seeded data (run it/redis/init_doctor_timeslot.py)
#   2. PostgreSQL with doctor_apm database
#   3. doctor-pool service running on port 8080

BASE_URL="${BASE_URL:-http://localhost:8080}"

echo "Testing doctor-pool timeslot API"
echo "================================"
echo ""

# Health check
echo "1. Health check:"
curl -s "${BASE_URL}/health"
echo -e "\n"

# Test doctor 321 - Sunday-Monday (2026-02-16 is Sunday, 2026-02-17 is Monday)
echo "2. Get timeslots for doctor 321 (Feb 16-17, 2026):"
echo "   GET ${BASE_URL}/doctors/321/timeslots?startDate=2026-02-16&endDate=2026-02-17&slotDurationMinutes=30"
curl -s "${BASE_URL}/doctors/321/timeslots?startDate=2026-02-16&endDate=2026-02-17&slotDurationMinutes=30" | python3 -m json.tool 2>/dev/null || echo "Service not running or invalid response"
echo ""

# Test doctor 427 - Weekdays (2026-02-17 is Monday, 2026-02-19 is Wednesday)
echo "3. Get timeslots for doctor 427 (Feb 17-19, 2026):"
echo "   GET ${BASE_URL}/doctors/427/timeslots?startDate=2026-02-17&endDate=2026-02-19&slotDurationMinutes=60"
curl -s "${BASE_URL}/doctors/427/timeslots?startDate=2026-02-17&endDate=2026-02-19&slotDurationMinutes=60" | python3 -m json.tool 2>/dev/null || echo "Service not running or invalid response"
echo ""

# Test with ad-hoc date for doctor 321 (2026-02-12)
echo "4. Get timeslots for doctor 321 with ad-hoc date (Feb 12, 2026):"
echo "   GET ${BASE_URL}/doctors/321/timeslots?startDate=2026-02-12&endDate=2026-02-12&slotDurationMinutes=30"
curl -s "${BASE_URL}/doctors/321/timeslots?startDate=2026-02-12&endDate=2026-02-12&slotDurationMinutes=30" | python3 -m json.tool 2>/dev/null || echo "Service not running or invalid response"
echo ""

# Test error case - invalid date format
echo "5. Test invalid date format:"
echo "   GET ${BASE_URL}/doctors/321/timeslots?startDate=2026-02-16&endDate=invalid"
curl -s "${BASE_URL}/doctors/321/timeslots?startDate=2026-02-16&endDate=invalid" | python3 -m json.tool 2>/dev/null || echo "Service not running or invalid response"
echo ""

# Test error case - end date before start date
echo "6. Test end date before start date:"
echo "   GET ${BASE_URL}/doctors/321/timeslots?startDate=2026-02-20&endDate=2026-02-10"
curl -s "${BASE_URL}/doctors/321/timeslots?startDate=2026-02-20&endDate=2026-02-10" | python3 -m json.tool 2>/dev/null || echo "Service not running or invalid response"
echo ""

echo "================================"
echo "Testing complete!"
