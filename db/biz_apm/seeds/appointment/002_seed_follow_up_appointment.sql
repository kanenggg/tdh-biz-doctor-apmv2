-- sqlfluff:dialect:postgres

WITH parent_reservation AS (
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
    ) VALUES (
        nextval('v2.reservation_booking_id_seq') + 20,
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
        now() - INTERVAL '2 hours',
        now() - INTERVAL '1 hour 30 minutes',
        now() - INTERVAL '1 hour 30 minutes'
    )
    RETURNING booking_id
),
parent_appointment AS (
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
    ) VALUES (
        (SELECT booking_id FROM parent_reservation),
        (SELECT booking_id FROM parent_reservation),
        0,
        NULL,
        'FULFILLED',
        now() - INTERVAL '2 hours',
        INTERVAL '30 minutes',
        now() - INTERVAL '1 hour 30 minutes',
        TRUE
    )
    RETURNING appointment_id
),
followup_reservation AS (
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
    ) VALUES (
        nextval('v2.reservation_booking_id_seq') + 21,
        1001,
        2001,
        3001,
        3001,
        4001,
        1,
        100,
        1,
        'FollowUp',
        'video',
        now() + INTERVAL '1 day',
        now() + INTERVAL '1 day' + INTERVAL '30 minutes',
        now() + INTERVAL '1 day' + INTERVAL '30 minutes'
    )
    RETURNING booking_id
)
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
) VALUES (
    (SELECT booking_id FROM followup_reservation),
    (SELECT booking_id FROM followup_reservation),
    0,
    (SELECT appointment_id FROM parent_appointment),
    'BOOKED',
    now() + INTERVAL '1 day',
    INTERVAL '30 minutes',
    now() + INTERVAL '1 day' + INTERVAL '30 minutes',
    FALSE
);
