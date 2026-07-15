mod common;

use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

fn hold_key(prefix: &str) -> String {
    format!("{}{}", prefix, Uuid::new_v4().simple())
}

async fn cleanup_hold(pool: &PgPool, idempotency_key: &str) {
    let _ = sqlx::query(
        r#"
        DELETE FROM v2.doctor_occupancy
        WHERE hold_id IN (
            SELECT hold_id FROM v2.appointment_hold WHERE idempotency_key = $1
        )
        "#,
    )
    .bind(idempotency_key)
    .execute(pool)
    .await;

    let _ = sqlx::query(
        r#"
        DELETE FROM v2.appointment
        WHERE source_hold_id IN (
            SELECT hold_id FROM v2.appointment_hold WHERE idempotency_key = $1
        )
        "#,
    )
    .bind(idempotency_key)
    .execute(pool)
    .await;

    let _ = sqlx::query("DELETE FROM v2.appointment_hold WHERE idempotency_key = $1")
        .bind(idempotency_key)
        .execute(pool)
        .await;
}

async fn insert_hold(
    pool: &PgPool,
    idempotency_key: &str,
    doctor_profile_id: i32,
    starts_at: &str,
    ends_at: &str,
    expires_at: &str,
    hold_status: &str,
    acceptance_status: &str,
) -> Result<Uuid, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        INSERT INTO v2.appointment_hold (
            booking_id,
            idempotency_key,
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
            starts_at,
            ends_at,
            expires_at,
            purpose_code,
            purpose_note,
            hold_status,
            payment_status,
            acceptance_status,
            payment_required,
            payment_tx_ref_id,
            payment_channels,
            accepted_at,
            accepted_by_account_id,
            acceptance_payload,
            prescreen_payload,
            prescreen_data_type
        ) VALUES (
            left(replace($1, '-', ''), 20),
            $1,
            10001,
            20001,
            30001,
            40001,
            $2,
            1,
            1,
            1,
            'Schedule'::v2.booking_type_enum,
            'video'::v2.consultation_type_enum,
            $3::timestamptz,
            $4::timestamptz,
            $5::timestamptz,
            'GENERAL_CONSULT',
            'schema integration test',
            $6::v2.appointment_hold_status_enum,
            'PENDING'::v2.appointment_hold_payment_status_enum,
            $7::v2.appointment_hold_acceptance_status_enum,
            true,
            $8,
            $9::jsonb,
            CASE
                WHEN $7::v2.appointment_hold_acceptance_status_enum = 'ACCEPTED'
                THEN $3::timestamptz
                ELSE NULL
            END,
            CASE
                WHEN $7::v2.appointment_hold_acceptance_status_enum = 'ACCEPTED'
                THEN 10001
                ELSE NULL
            END,
            $10::jsonb,
            $11::jsonb,
            'RAW_JSON'
        ) RETURNING hold_id
        "#,
    )
    .bind(idempotency_key)
    .bind(doctor_profile_id)
    .bind(starts_at)
    .bind(ends_at)
    .bind(expires_at)
    .bind(hold_status)
    .bind(acceptance_status)
    .bind(format!("pay-{idempotency_key}"))
    .bind(json!([{"channel":"card"}]))
    .bind(json!({"accepted": acceptance_status == "ACCEPTED", "source": "test"}))
    .bind(json!({"symptom":"cough", "duration": 2, "unit":"day"}))
    .fetch_one(pool)
    .await
}

#[tokio::test]
async fn create_active_appointment_hold_with_prescreen_purpose_status_payment_acceptance_fields()
-> Result<(), anyhow::Error> {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    let idempotency_key = hold_key("hold-active-");
    cleanup_hold(pool, &idempotency_key).await;

    let hold_id = insert_hold(
        pool,
        &idempotency_key,
        810001,
        "2026-08-01T02:00:00Z",
        "2026-08-01T02:30:00Z",
        "2026-07-31T02:00:00Z",
        "ACTIVE",
        "PENDING",
    )
    .await?;

    let row: (
        String,
        String,
        String,
        String,
        bool,
        bool,
        serde_json::Value,
        serde_json::Value,
    ) = sqlx::query_as(
        r#"
        SELECT
            purpose_code,
            hold_status::text,
            payment_status::text,
            acceptance_status::text,
            expires_at < starts_at,
            payment_required,
            acceptance_payload,
            prescreen_payload
        FROM v2.appointment_hold
        WHERE hold_id = $1
        "#,
    )
    .bind(hold_id)
    .fetch_one(pool)
    .await?;

    assert_eq!(row.0, "GENERAL_CONSULT");
    assert_eq!(row.1, "ACTIVE");
    assert_eq!(row.2, "PENDING");
    assert_eq!(row.3, "PENDING");
    assert!(row.4, "hold expiry may be before appointment start");
    assert!(row.5);
    assert_eq!(row.6["accepted"], false);
    assert_eq!(row.7["symptom"], "cough");

    cleanup_hold(pool, &idempotency_key).await;
    Ok(())
}

#[tokio::test]
async fn overlapping_active_holds_for_same_doctor_conflict() -> Result<(), anyhow::Error> {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    let first_key = hold_key("hold-overlap-a-");
    let second_key = hold_key("hold-overlap-b-");
    cleanup_hold(pool, &first_key).await;
    cleanup_hold(pool, &second_key).await;

    let first_hold_id = insert_hold(
        pool,
        &first_key,
        810002,
        "2026-08-02T02:00:00Z",
        "2026-08-02T02:30:00Z",
        "2026-08-01T02:00:00Z",
        "ACTIVE",
        "PENDING",
    )
    .await?;
    sqlx::query(
        "INSERT INTO v2.doctor_occupancy (doctor_profile_id, starts_at, ends_at, hold_id) VALUES (810002, '2026-08-02T02:00:00Z', '2026-08-02T02:30:00Z', $1)",
    )
    .bind(first_hold_id)
    .execute(pool)
    .await?;

    let second_hold_id = insert_hold(
        pool,
        &second_key,
        810002,
        "2026-08-02T02:15:00Z",
        "2026-08-02T02:45:00Z",
        "2026-08-01T02:15:00Z",
        "ACTIVE",
        "PENDING",
    )
    .await?;
    let conflict = sqlx::query(
        "INSERT INTO v2.doctor_occupancy (doctor_profile_id, starts_at, ends_at, hold_id) VALUES (810002, '2026-08-02T02:15:00Z', '2026-08-02T02:45:00Z', $1)",
    )
    .bind(second_hold_id)
    .execute(pool)
    .await;

    assert!(
        conflict.is_err(),
        "overlapping ACTIVE Doctor Occupancy must fail"
    );

    cleanup_hold(pool, &first_key).await;
    cleanup_hold(pool, &second_key).await;
    Ok(())
}

#[tokio::test]
async fn released_and_expired_holds_do_not_block_new_active_hold() -> Result<(), anyhow::Error> {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    let released_key = hold_key("hold-released-");
    let expired_key = hold_key("hold-expired-");
    let active_key = hold_key("hold-after-inactive-");
    cleanup_hold(pool, &released_key).await;
    cleanup_hold(pool, &expired_key).await;
    cleanup_hold(pool, &active_key).await;

    insert_hold(
        pool,
        &released_key,
        810003,
        "2026-08-03T02:00:00Z",
        "2026-08-03T02:30:00Z",
        "2026-08-02T02:00:00Z",
        "RELEASED",
        "CANCELLED",
    )
    .await?;
    insert_hold(
        pool,
        &expired_key,
        810003,
        "2026-08-03T02:15:00Z",
        "2026-08-03T02:45:00Z",
        "2026-08-02T02:15:00Z",
        "EXPIRED",
        "PENDING",
    )
    .await?;

    let active_hold_id = insert_hold(
        pool,
        &active_key,
        810003,
        "2026-08-03T02:10:00Z",
        "2026-08-03T02:40:00Z",
        "2026-08-02T02:10:00Z",
        "ACTIVE",
        "PENDING",
    )
    .await?;

    assert_ne!(active_hold_id, Uuid::nil());

    cleanup_hold(pool, &released_key).await;
    cleanup_hold(pool, &expired_key).await;
    cleanup_hold(pool, &active_key).await;
    Ok(())
}

#[tokio::test]
async fn acceptance_status_is_separate_from_lifecycle_and_payment_status()
-> Result<(), anyhow::Error> {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    let pending_key = hold_key("hold-accept-pending-");
    let accepted_key = hold_key("hold-accept-accepted-");
    let declined_key = hold_key("hold-accept-declined-");
    cleanup_hold(pool, &pending_key).await;
    cleanup_hold(pool, &accepted_key).await;
    cleanup_hold(pool, &declined_key).await;

    let pending_id = insert_hold(
        pool,
        &pending_key,
        810005,
        "2026-08-05T02:00:00Z",
        "2026-08-05T02:30:00Z",
        "2026-08-04T02:00:00Z",
        "ACTIVE",
        "PENDING",
    )
    .await?;
    let accepted_id = insert_hold(
        pool,
        &accepted_key,
        810006,
        "2026-08-05T03:00:00Z",
        "2026-08-05T03:30:00Z",
        "2026-08-04T03:00:00Z",
        "ACTIVE",
        "ACCEPTED",
    )
    .await?;
    let declined_id = insert_hold(
        pool,
        &declined_key,
        810007,
        "2026-08-05T04:00:00Z",
        "2026-08-05T04:30:00Z",
        "2026-08-04T04:00:00Z",
        "ACTIVE",
        "DECLINED",
    )
    .await?;

    let rows: Vec<(String, String, String, bool)> = sqlx::query_as(
        r#"
        SELECT hold_status::text, payment_status::text, acceptance_status::text, accepted_at IS NOT NULL
        FROM v2.appointment_hold
        WHERE hold_id = ANY($1)
        ORDER BY acceptance_status::text
        "#,
    )
    .bind(vec![pending_id, accepted_id, declined_id])
    .fetch_all(pool)
    .await?;

    assert!(rows.contains(&("ACTIVE".into(), "PENDING".into(), "PENDING".into(), false)));
    assert!(rows.contains(&("ACTIVE".into(), "PENDING".into(), "ACCEPTED".into(), true)));
    assert!(rows.contains(&("ACTIVE".into(), "PENDING".into(), "DECLINED".into(), false)));

    cleanup_hold(pool, &pending_key).await;
    cleanup_hold(pool, &accepted_key).await;
    cleanup_hold(pool, &declined_key).await;
    Ok(())
}

#[tokio::test]
async fn booked_appointment_can_reference_source_hold_and_prescreen() -> Result<(), anyhow::Error> {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    let idempotency_key = hold_key("hold-booked-");
    let booking_id = format!("AH{}", &Uuid::new_v4().simple().to_string()[..18]);
    cleanup_hold(pool, &idempotency_key).await;
    let _ = sqlx::query("DELETE FROM v2.appointment WHERE appointment_id = $1 OR booking_id = $1")
        .bind(&booking_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM v2.patient_prescreen WHERE booking_id = $1")
        .bind(&booking_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM v2.reservation WHERE booking_id = $1")
        .bind(&booking_id)
        .execute(pool)
        .await;

    let hold_id = insert_hold(
        pool,
        &idempotency_key,
        810004,
        "2026-08-04T02:00:00Z",
        "2026-08-04T02:30:00Z",
        "2026-08-03T02:00:00Z",
        "ACTIVE",
        "ACCEPTED",
    )
    .await?;

    sqlx::query(
        r#"
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
            reserved_until,
            booking_type,
            consultation_channel,
            appointment_start,
            appointment_end
        ) VALUES (
            $1,
            10001,
            20001,
            30001,
            40001,
            810004,
            1,
            1,
            1,
            'CONFIRMED'::v2.reservation_status_enum,
            '2026-08-04T02:30:00Z'::timestamptz,
            'Schedule'::v2.booking_type_enum,
            'video'::v2.consultation_type_enum,
            '2026-08-04T02:00:00Z'::timestamptz,
            '2026-08-04T02:30:00Z'::timestamptz
        )
        "#,
    )
    .bind(&booking_id)
    .execute(pool)
    .await?;

    let prescreen_id: i32 = sqlx::query_scalar(
        r#"
        INSERT INTO v2.patient_prescreen (
            booking_id,
            prescreen_data,
            prescreen_data_type,
            user_account_id,
            user_profile_id
        ) VALUES ($1, $2, 'RAW_JSON', 10001, 20001)
        RETURNING prescreen_id
        "#,
    )
    .bind(&booking_id)
    .bind(json!({"symptom":"cough"}).to_string())
    .fetch_one(pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO v2.appointment (
            appointment_id,
            booking_id,
            prescreen_data_id,
            source_hold_id,
            source_hold_prescreen_id,
            appointment_status,
            appointment_start,
            consult_duration,
            appointment_end,
            has_follow_up
        ) VALUES (
            $1,
            $1,
            $2,
            $3,
            $2,
            'BOOKED'::v2.fhir_appointment_status_enum,
            '2026-08-04T02:00:00Z'::timestamptz,
            INTERVAL '30 minutes',
            '2026-08-04T02:30:00Z'::timestamptz,
            false
        )
        "#,
    )
    .bind(&booking_id)
    .bind(prescreen_id)
    .bind(hold_id)
    .execute(pool)
    .await?;

    let linked: (Uuid, i32, String) = sqlx::query_as(
        r#"
        SELECT source_hold_id, source_hold_prescreen_id, appointment_status::text
        FROM v2.appointment
        WHERE appointment_id = $1
        "#,
    )
    .bind(&booking_id)
    .fetch_one(pool)
    .await?;

    assert_eq!(linked.0, hold_id);
    assert_eq!(linked.1, prescreen_id);
    assert_eq!(linked.2, "BOOKED");

    let _ = sqlx::query("DELETE FROM v2.appointment WHERE appointment_id = $1 OR booking_id = $1")
        .bind(&booking_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM v2.patient_prescreen WHERE booking_id = $1")
        .bind(&booking_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM v2.reservation WHERE booking_id = $1")
        .bind(&booking_id)
        .execute(pool)
        .await;
    cleanup_hold(pool, &idempotency_key).await;
    Ok(())
}
