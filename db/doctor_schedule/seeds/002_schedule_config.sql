-- sqlfluff:dialect:postgres

-- Seed data for doctor_schedule table
-- Creates 50 doctors with varied schedule configurations
\c biz_mordee_doctor;

INSERT INTO doctor_schedule (doctor_id, schedule_config, created_at)
SELECT 
    generate_uuid_v7(),
    CASE 
        WHEN n % 5 = 0 THEN '{
            "routine": [
                {
                    "dayOfWeek": 0,
                    "times": [
                        {"startTime": "09:00", "endTime": "11:00"},
                        {"startTime": "14:00", "endTime": "17:00"}
                    ]
                },
                {
                    "dayOfWeek": 1,
                    "times": [
                        {"startTime": "10:00", "endTime": "12:00"}
                    ]
                }
            ],
            "adHoc": [
                {"date": "2026-02-15", "times": null}
            ]
        }'::JSONB
        WHEN n % 5 = 1 THEN '{
            "routine": [
                {
                    "dayOfWeek": 0,
                    "times": [
                        {"startTime": "10:00", "endTime": "12:00"},
                        {"startTime": "14:00", "endTime": "16:00"}
                    ]
                },
                {
                    "dayOfWeek": 0,
                    "times": [
                        {"startTime": "17:00", "endTime": "18:00"}
                    ]
                }
            ],
            "adHoc": [
                {
                    "date": "2026-02-15",
                    "times": [
                        {"startTime": "10:00", "endTime": "12:00"},
                        {"startTime": "14:00", "endTime": "16:00"}
                    ]
                },
                {"date": "2026-02-15", "times": null}
            ]
        }'::JSONB
        WHEN n % 5 = 2 THEN '{
            "routine": [
                {
                    "dayOfWeek": 0,
                    "times": [
                        {"startTime": "08:00", "endTime": "10:00"},
                        {"startTime": "13:00", "endTime": "15:00"}
                    ]
                },
                {
                    "dayOfWeek": 2,
                    "times": [
                        {"startTime": "16:00", "endTime": "19:00"}
                    ]
                }
            ],
            "adHoc": [
                {"date": "2026-02-16", "times": null}
            ]
        }'::JSONB
        WHEN n % 5 = 3 THEN '{
            "routine": [
                {
                    "dayOfWeek": 1,
                    "times": [
                        {"startTime": "09:00", "endTime": "13:00"}
                    ]
                },
                {
                    "dayOfWeek": 3,
                    "times": [
                        {"startTime": "15:00", "endTime": "18:00"}
                    ]
                }
            ],
            "adHoc": [
                {"date": "2026-02-17", "times": null}
            ]
        }'::JSONB
        ELSE '{
            "routine": [
                {
                    "dayOfWeek": 0,
                    "times": [
                        {"startTime": "11:00", "endTime": "13:00"},
                        {"startTime": "15:00", "endTime": "17:00"}
                    ]
                },
                {
                    "dayOfWeek": 4,
                    "times": [
                        {"startTime": "10:00", "endTime": "12:00"},
                        {"startTime": "14:00", "endTime": "16:00"}
                    ]
                }
            ],
            "adHoc": [
                {
                    "date": "2026-02-18",
                    "times": [
                        {"startTime": "10:00", "endTime": "12:00"}
                    ]
                }
            ]
        }'::JSONB
    END AS schedule_config,
    NOW()
FROM generate_series(1, 50) AS n
ON CONFLICT (doctor_id) DO NOTHING;
