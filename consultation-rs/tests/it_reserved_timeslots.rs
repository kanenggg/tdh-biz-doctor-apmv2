mod common;

use consultation_rs::doctor_timeslot::reserved_timeslot::{
    repo::{ReservedTimeslotsRepo, ReservedTimeslotsRepoPsql},
    service::bkk_day_range,
};
use sqlx::PgPool;

/// Insert a bare-minimum reservation row into v2.reservation.
/// `status` must be a valid v2.reservation_status_enum value (e.g. 'RESERVED', 'CANCELLED').
async fn seed_reservation(
    pool: &PgPool,
    booking_id: &str,
    doctor_profile_id: i32,
    status: &str,
    start: &str,
    end: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO v2.reservation (
            booking_id,
            patient_account_id, patient_profile_id,
            doctor_id, doctor_account_id, doctor_profile_id,
            biz_unit_id, tenant_id,
            reservation_status, reserved_until,
            booking_type, consultation_channel,
            appointment_start, appointment_end
        ) VALUES (
            $1,
            10001, 20001,
            99, 30001, $2,
            1, 1,
            $3::v2.reservation_status_enum, $5::timestamptz,
            'Schedule'::v2.booking_type_enum, 'video'::v2.consultation_type_enum,
            $4::timestamptz, $5::timestamptz
        )
        "#,
    )
    .bind(booking_id)
    .bind(doctor_profile_id)
    .bind(status)
    .bind(start)
    .bind(end)
    .execute(pool)
    .await
    .expect("Failed to insert reservation");
}

async fn cleanup(pool: &PgPool, booking_id: &str) {
    sqlx::query("DELETE FROM v2.reservation WHERE booking_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
}

/// The repo must return exactly the RESERVED in-day row and exclude:
/// - CANCELLED rows (even if in-day)
/// - RESERVED rows outside the day window (next day)
#[tokio::test]
#[ignore]
async fn test_reserved_timeslots_filters_cancelled_and_next_day() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let doctor_profile_id: i32 = 777;
    let ids = ["BKTS_R1", "BKTS_C1", "BKTS_R2"];

    for id in ids {
        cleanup(pool, id).await;
    }

    // Row 1: RESERVED inside 2026-06-18 BKK (09:00–09:30 BKK = 02:00–02:30 UTC)
    seed_reservation(
        pool,
        "BKTS_R1",
        doctor_profile_id,
        "RESERVED",
        "2026-06-18T02:00:00Z",
        "2026-06-18T02:30:00Z",
    )
    .await;

    // Row 2: CANCELLED inside 2026-06-18 BKK — must be excluded
    seed_reservation(
        pool,
        "BKTS_C1",
        doctor_profile_id,
        "CANCELLED",
        "2026-06-18T03:00:00Z",
        "2026-06-18T03:30:00Z",
    )
    .await;

    // Row 3: RESERVED on 2026-06-19 BKK (next day) — must be excluded
    seed_reservation(
        pool,
        "BKTS_R2",
        doctor_profile_id,
        "RESERVED",
        "2026-06-19T02:00:00Z",
        "2026-06-19T02:30:00Z",
    )
    .await;

    let (day_start, day_end) = bkk_day_range("2026-06-18").expect("bkk_day_range failed");
    let repo = ReservedTimeslotsRepoPsql::new(pool.clone());
    let results = repo
        .find_reserved_timeslots_by_doctor_profile(doctor_profile_id, day_start, day_end)
        .await
        .expect("repo query failed");

    assert_eq!(
        results.len(),
        1,
        "expected exactly 1 timeslot, got {}: {:?}",
        results.len(),
        results
    );

    let slot = &results[0];
    assert_eq!(slot.booking_id, "BKTS_R1");

    // 2026-06-18T02:00:00Z in epoch seconds
    let expected_start: i64 = 1781748000;
    // 2026-06-18T02:30:00Z in epoch seconds
    let expected_end: i64 = 1781749800;
    assert_eq!(
        slot.start_time, expected_start,
        "start_time mismatch: got {}, want {}",
        slot.start_time, expected_start
    );
    assert_eq!(
        slot.end_time, expected_end,
        "end_time mismatch: got {}, want {}",
        slot.end_time, expected_end
    );

    for id in ids {
        cleanup(pool, id).await;
    }
    println!("✓ test_reserved_timeslots_filters_cancelled_and_next_day passed");
}
