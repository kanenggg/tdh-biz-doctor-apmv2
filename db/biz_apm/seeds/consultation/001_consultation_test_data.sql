-- sqlfluff:dialect:postgres

-- Seed data for consultation service testing
-- This creates test bookings, appointments, and session info for development

-- Insert test reservations
INSERT INTO v2.reservation (
    booking_id,
    patient_account_id,
    patient_profile_id,
    doctor_id,
    doctor_account_id,
    doctor_profile_id,
    biz_unit_id,
    biz_center_id,
    tenant_id,
    booking_type,
    consultation_channel,
    appointment_start,
    appointment_end,
    reserved_until
) OVERRIDING SYSTEM VALUE VALUES
(
    '10001',
    1001,
    2001,
    3001,
    3001,
    4001,
    1,
    100,
    1,
    'Instant',
    'video',
    now() - INTERVAL '30 minutes',
    now() + INTERVAL '30 minutes',
    now() + INTERVAL '30 minutes'
),
(
    '10002',
    1002,
    2002,
    3002,
    3002,
    4002,
    2,
    101,
    1,
    'Schedule',
    'voice',
    now() - INTERVAL '2 hours',
    now() - INTERVAL '1 hour',
    now() - INTERVAL '1 hour'
),
(
    '10003',
    1001,
    2001,
    3003,
    3003,
    4003,
    1,
    100,
    1,
    'FollowUp',
    'video',
    now() + INTERVAL '2 hours',
    now() + INTERVAL '3 hours',
    now() + INTERVAL '3 hours'
)
ON CONFLICT (booking_id) DO NOTHING;

-- Insert test prescreen data
INSERT INTO v2.patient_prescreen (booking_id, prescreen_data, prescreen_data_type, user_account_id, user_profile_id)
VALUES
    ('10001', '{}', 'TEXT', 1001, 2001),
    ('10002', '{}', 'TEXT', 1002, 2002),
    ('10003', '{}', 'TEXT', 1001, 2001)
ON CONFLICT DO NOTHING;

-- Insert test appointments
INSERT INTO v2.appointment (
    appointment_id,
    booking_id,
    prescreen_data_id,
    parent_appointment_id,
    appointment_status,
    appointment_start,
    consult_duration,
    appointment_end,
    has_follow_up
) VALUES
(
    '10001',
    '10001',
    0,
    NULL,
    'BOOKED',
    now() - INTERVAL '30 minutes',
    INTERVAL '1 hour',
    now() + INTERVAL '30 minutes',
    FALSE
),
(
    '10002',
    '10002',
    0,
    NULL,
    'FULFILLED',
    now() - INTERVAL '2 hours',
    INTERVAL '1 hour',
    now() - INTERVAL '1 hour',
    FALSE
),
(
    '10003',
    '10003',
    0,
    '10001',
    'BOOKED',
    now() + INTERVAL '2 hours',
    INTERVAL '1 hour',
    now() + INTERVAL '3 hours',
    FALSE
)
ON CONFLICT (appointment_id) DO NOTHING;

-- Insert test session_info (delete existing first to avoid duplicates)
DELETE FROM v2.session_info
WHERE appointment_id IN ('10001', '10002');

INSERT INTO v2.session_info (
    appointment_id,
    session_provider,
    session_data
) VALUES
(
    '10001',
    'TWILIO',
    '{
        "__type": "twilio",
        "recordingUrl": "https://example.com/recording/10001",
        "sessionChatId": "CH001",
        "chatRecordingUrl": "https://example.com/chat-recording/10001",
        "sessionChatServiceId": "IS001"
    }'::JSONB
),
(
    '10002',
    'TWILIO',
    '{
        "__type": "twilio",
        "recordingUrl": "https://example.com/recording/10002",
        "sessionChatId": "CH002",
        "chatRecordingUrl": "https://example.com/chat-recording/10002",
        "sessionChatServiceId": "IS002"
    }'::JSONB
);

-- Insert test payment transactions
INSERT INTO v2.appointment_payment_transaction (appointment_id, payment_tx_ref_id, payment_channels)
SELECT '10001', 'tx-ref-001', '[{"__type":"Card","id":"124"}]'::jsonb
WHERE NOT EXISTS (
    SELECT 1 FROM v2.appointment_payment_transaction WHERE appointment_id = '10001'
);

INSERT INTO v2.appointment_payment_transaction (appointment_id, payment_tx_ref_id, payment_channels)
SELECT '10002', 'tx-ref-002', '[{"__type":"Insurance","binding_id": 12345, "privilege_id": 1}]'::jsonb
WHERE NOT EXISTS (
    SELECT 1 FROM v2.appointment_payment_transaction WHERE appointment_id = '10002'
);
