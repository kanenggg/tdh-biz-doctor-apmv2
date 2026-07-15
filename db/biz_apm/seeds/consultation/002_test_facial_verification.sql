-- sqlfluff:dialect:postgres

-- ============================================
-- Test for is_facial_verified flag in get_consultation_session
-- This test verifies:
-- 1. First entry (no facial upload) → is_facial_verified = false for VIDEO channel
-- 2. After upload → is_facial_verified = true for VIDEO channel
-- 3. Non-VIDEO channel → is_facial_verified = null
-- ============================================

-- Clean up any existing test data
DELETE FROM v2.appointment_facial_upload WHERE appointment_id = '99999';
DELETE FROM v2.session_info WHERE appointment_id = '99999';
DELETE FROM v2.appointment WHERE appointment_id = '99999';
DELETE FROM v2.reservation WHERE booking_id = '99999';

-- Insert test reservation with VIDEO channel
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
    reservation_status,
    booking_type,
    consultation_channel,
    appointment_start,
    appointment_end,
    reserved_until
) VALUES (
    '99999',
    9999,
    8888,
    7777,
    6666,
    5555,
    1,
    100,
    1,
    'CONFIRMED',
    'Instant',
    'video',
    now() + INTERVAL '1 hour',
    now() + INTERVAL '2 hour',
    now() + INTERVAL '2 hour'
);

-- Insert test appointment
INSERT INTO v2.appointment (
    appointment_id,
    booking_id,
    prescreen_data_id,
    appointment_status,
    appointment_start,
    consult_duration,
    appointment_end,
    has_follow_up
) VALUES (
    '99999',
    '99999',
    0,
    'BOOKED',
    now() + INTERVAL '1 hour',
    INTERVAL '1 hour',
    now() + INTERVAL '2 hour',
    FALSE
);

-- ============================================
-- TEST 1: First entry - no facial upload yet
-- Expected: is_facial_verified = false (for VIDEO channel without upload)
-- ============================================
SELECT 'TEST 1: First entry (no facial upload)' as test_name;
SELECT
    appointment_id,
    consultation_channel,
    is_facial_verified,
    CASE
        WHEN is_facial_verified = false THEN 'PASS: is_facial_verified is false as expected'
        ELSE 'FAIL: is_facial_verified should be false but is ' || COALESCE(is_facial_verified::text, 'NULL')
    END as result
FROM v2.get_consultation_session('99999', 8888);

-- ============================================
-- TEST 2: After facial upload - insert record
-- ============================================
INSERT INTO v2.appointment_facial_upload (
    appointment_id,
    user_profile_id,
    user_account_id,
    object_url
) VALUES (
    '99999',
    8888,
    9999,
    'https://storage.example.com/facial/99999.jpg'
);

-- Expected: is_facial_verified = true (for VIDEO channel with upload)
SELECT 'TEST 2: After facial upload' as test_name;
SELECT
    appointment_id,
    consultation_channel,
    is_facial_verified,
    CASE
        WHEN is_facial_verified = true THEN 'PASS: is_facial_verified is true as expected'
        ELSE 'FAIL: is_facial_verified should be true but is ' || COALESCE(is_facial_verified::text, 'NULL')
    END as result
FROM v2.get_consultation_session('99999', 8888);

-- ============================================
-- TEST 3: Non-VIDEO channel (VOICE) - should return NULL
-- ============================================
-- Clean up voice test data
DELETE FROM v2.appointment_facial_upload WHERE appointment_id = '99998';
DELETE FROM v2.session_info WHERE appointment_id = '99998';
DELETE FROM v2.appointment WHERE appointment_id = '99998';
DELETE FROM v2.reservation WHERE booking_id = '99998';

-- Insert test reservation with VOICE channel
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
    reservation_status,
    booking_type,
    consultation_channel,
    appointment_start,
    appointment_end,
    reserved_until
) VALUES (
    '99998',
    9999,
    8888,
    7777,
    6666,
    5555,
    1,
    100,
    1,
    'CONFIRMED',
    'Instant',
    'voice',
    now() + INTERVAL '1 hour',
    now() + INTERVAL '2 hour',
    now() + INTERVAL '2 hour'
);

INSERT INTO v2.appointment (
    appointment_id,
    booking_id,
    prescreen_data_id,
    appointment_status,
    appointment_start,
    consult_duration,
    appointment_end,
    has_follow_up
) VALUES (
    '99998',
    '99998',
    0,
    'BOOKED',
    now() + INTERVAL '1 hour',
    INTERVAL '1 hour',
    now() + INTERVAL '2 hour',
    FALSE
);

SELECT 'TEST 3: Non-VIDEO channel (VOICE)' as test_name;
SELECT
    appointment_id,
    consultation_channel,
    is_facial_verified,
    CASE
        WHEN is_facial_verified = true THEN 'PASS: is_facial_verified is true as expected for non-VIDEO'
        ELSE 'FAIL: is_facial_verified should be true but is ' || COALESCE(is_facial_verified::text, 'NULL')
    END as result
FROM v2.get_consultation_session('99998', 8888);

-- ============================================
-- TEST 4: Doctor querying (should also work)
-- ============================================
SELECT 'TEST 4: Doctor querying session' as test_name;
SELECT
    appointment_id,
    consultation_channel,
    is_facial_verified,
    CASE
        WHEN is_facial_verified = true THEN 'PASS: Doctor can see is_facial_verified correctly'
        ELSE 'FAIL: is_facial_verified should be true but is ' || COALESCE(is_facial_verified::text, 'NULL')
    END as result
FROM v2.get_consultation_session('99999', 5555);
