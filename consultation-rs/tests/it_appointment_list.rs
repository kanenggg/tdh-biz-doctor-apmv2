mod common;

use std::sync::Arc;

use consultation_rs::appointment::list::{
    model::ListAppointmentsResponse, repo::ListAppointmentsRepoPsql,
    service::ListAppointmentsService,
};
use sqlx::PgPool;

/// Insert one reservation + appointment pair with the given identity, status, and times.
/// Mirrors the seeding style in `it_appointment_get_detail.rs` (reservation carries the
/// patient/doctor ids and times; appointment carries the FHIR status).
async fn seed_appointment(
    pool: &PgPool,
    booking_id: &str,
    patient_account_id: i32,
    patient_profile_id: i32,
    doctor_account_id: i32,
    doctor_profile_id: i32,
    status: &str,
    start: &str,
    end: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO v2.reservation (
            booking_id, patient_account_id, patient_profile_id,
            doctor_id, doctor_account_id, doctor_profile_id,
            biz_unit_id, tenant_id, reservation_status, reserved_until,
            booking_type, consultation_channel, appointment_start, appointment_end
        ) VALUES (
            $1, $2, $3, 99, $4, $5, 1, 1,
            'CONFIRMED'::v2.reservation_status_enum, $7::timestamptz,
            'Schedule'::v2.booking_type_enum, 'video'::v2.consultation_type_enum,
            $6::timestamptz, $7::timestamptz
        )
        "#,
    )
    .bind(booking_id)
    .bind(patient_account_id)
    .bind(patient_profile_id)
    .bind(doctor_account_id)
    .bind(doctor_profile_id)
    .bind(start)
    .bind(end)
    .execute(pool)
    .await
    .expect("Failed to insert reservation");

    sqlx::query(
        r#"
        INSERT INTO v2.appointment (
            appointment_id, booking_id, prescreen_data_id, appointment_status,
            appointment_start, appointment_end, consult_duration, has_follow_up
        ) VALUES (
            $1, $1, 99999, $2::v2.fhir_appointment_status_enum,
            $3::timestamptz, $4::timestamptz, '15 minutes'::interval, false
        )
        "#,
    )
    .bind(booking_id)
    .bind(status)
    .bind(start)
    .bind(end)
    .execute(pool)
    .await
    .expect("Failed to insert appointment");
}

async fn cleanup(pool: &PgPool, booking_id: &str) {
    sqlx::query("DELETE FROM v2.appointment WHERE booking_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.reservation WHERE booking_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
}

fn success(
    resp: ListAppointmentsResponse,
) -> consultation_rs::appointment::list::model::AppointmentList {
    match resp {
        ListAppointmentsResponse::Success(list) => list,
    }
}

/// Account-only filter returns FULFILLED appointments across all profiles of the
/// account, newest first, and excludes non-FULFILLED rows.
#[tokio::test]
#[ignore]
async fn test_list_account_filter_returns_only_fulfilled_newest_first() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let account_id = 71001;
    let ids = ["BKLIST_A1", "BKLIST_A2", "BKLIST_A3", "BKLIST_A4"];
    for id in ids {
        cleanup(pool, id).await;
    }

    // Two profiles under the same account, both FULFILLED, at different times.
    seed_appointment(
        pool,
        "BKLIST_A1",
        account_id,
        81001,
        30001,
        40001,
        "FULFILLED",
        "2026-03-01T09:30:00Z",
        "2026-03-01T09:45:00Z",
    )
    .await;
    seed_appointment(
        pool,
        "BKLIST_A2",
        account_id,
        81002,
        30002,
        40002,
        "FULFILLED",
        "2026-04-01T09:30:00Z",
        "2026-04-01T09:45:00Z",
    )
    .await;
    // Same account but NOT fulfilled — must be excluded.
    seed_appointment(
        pool,
        "BKLIST_A3",
        account_id,
        81001,
        30003,
        40003,
        "BOOKED",
        "2026-05-01T09:30:00Z",
        "2026-05-01T09:45:00Z",
    )
    .await;
    // Different account — must be excluded.
    seed_appointment(
        pool,
        "BKLIST_A4",
        79999,
        81001,
        30004,
        40004,
        "FULFILLED",
        "2026-06-01T09:30:00Z",
        "2026-06-01T09:45:00Z",
    )
    .await;

    let service =
        ListAppointmentsService::new(Arc::new(ListAppointmentsRepoPsql::new(pool.clone())));
    let list = success(
        service
            .list_appointments(account_id, None)
            .await
            .expect("query should succeed"),
    );

    let booking_ids: Vec<&str> = list
        .appointments
        .iter()
        .map(|a| a.booking_id.as_str())
        .collect();
    // Only the two FULFILLED rows for this account, newest (April) first.
    assert_eq!(booking_ids, vec!["BKLIST_A2", "BKLIST_A1"]);

    // Doctor IDs are real; names are mocked (last_name derived from account id).
    let first = &list.appointments[0];
    assert_eq!(first.doctor.account_id, 30002);
    assert_eq!(first.doctor.last_name, "#30002");

    for id in ids {
        cleanup(pool, id).await;
    }
    println!("✓ test_list_account_filter_returns_only_fulfilled_newest_first passed");
}

/// Adding profile_id narrows the result to that single profile.
#[tokio::test]
#[ignore]
async fn test_list_profile_filter_narrows() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let account_id = 72001;
    let ids = ["BKLIST_P1", "BKLIST_P2"];
    for id in ids {
        cleanup(pool, id).await;
    }

    seed_appointment(
        pool,
        "BKLIST_P1",
        account_id,
        82001,
        30001,
        40001,
        "FULFILLED",
        "2026-03-01T09:30:00Z",
        "2026-03-01T09:45:00Z",
    )
    .await;
    seed_appointment(
        pool,
        "BKLIST_P2",
        account_id,
        82002,
        30002,
        40002,
        "FULFILLED",
        "2026-04-01T09:30:00Z",
        "2026-04-01T09:45:00Z",
    )
    .await;

    let service =
        ListAppointmentsService::new(Arc::new(ListAppointmentsRepoPsql::new(pool.clone())));
    let list = success(
        service
            .list_appointments(account_id, Some(82002))
            .await
            .expect("query should succeed"),
    );

    let booking_ids: Vec<&str> = list
        .appointments
        .iter()
        .map(|a| a.booking_id.as_str())
        .collect();
    assert_eq!(booking_ids, vec!["BKLIST_P2"]);

    for id in ids {
        cleanup(pool, id).await;
    }
    println!("✓ test_list_profile_filter_narrows passed");
}

/// Unknown patient → Success with an empty list (not an error, not 404).
#[tokio::test]
#[ignore]
async fn test_list_unknown_patient_returns_empty() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let service =
        ListAppointmentsService::new(Arc::new(ListAppointmentsRepoPsql::new(pool.clone())));
    let list = success(
        service
            .list_appointments(70000_0001, None)
            .await
            .expect("query should succeed"),
    );

    assert!(list.appointments.is_empty());
    println!("✓ test_list_unknown_patient_returns_empty passed");
}

/// More than 50 fulfilled appointments → capped at 50 (hard LIMIT, no pagination yet).
#[tokio::test]
#[ignore]
async fn test_list_limit_caps_at_50() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let account_id = 73001;
    let mut ids = Vec::new();
    // 51 fulfilled appointments, minute-spaced so ordering is well-defined.
    for i in 0..51 {
        let id = format!("BKLIST_L{i:02}");
        cleanup(pool, &id).await;
        let start = format!("2026-03-01T09:{:02}:00Z", i % 60);
        let end = format!("2026-03-01T09:{:02}:30Z", i % 60);
        seed_appointment(
            pool,
            &id,
            account_id,
            83001,
            30000 + i,
            40000 + i,
            "FULFILLED",
            &start,
            &end,
        )
        .await;
        ids.push(id);
    }

    let service =
        ListAppointmentsService::new(Arc::new(ListAppointmentsRepoPsql::new(pool.clone())));
    let list = success(
        service
            .list_appointments(account_id, None)
            .await
            .expect("query should succeed"),
    );

    assert_eq!(list.appointments.len(), 50);

    for id in &ids {
        cleanup(pool, id).await;
    }
    println!("✓ test_list_limit_caps_at_50 passed");
}
