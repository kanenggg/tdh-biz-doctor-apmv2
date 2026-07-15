use common_rs::tdh_protocol::{
    appointment::reserve::{PatientPrescreen, ReserveRequest, Timeslot},
    consultation::{BookingType, ConsultationChannel},
    iam::user_identity::{AccountType, UserIdentity},
};
use consultation_rs::{
    appointment::hold::{
        model::CreateAppointmentHold,
        repo::{AppointmentHoldPsql, AppointmentHoldRepo},
    },
    booking::repo::{BookingRepo, BookingRepoPsql},
};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

mod common;

const TEST_TOPIC: &str = "test-consultation-events";

fn unique_suffix() -> i32 {
    (Uuid::new_v4().as_u128() % 1_000_000) as i32
}

fn short_booking_id(prefix: &str) -> String {
    let simple = Uuid::new_v4().simple().to_string();
    format!("{}{}", prefix, &simple[..18])
}

fn test_user(suffix: i32) -> UserIdentity {
    UserIdentity {
        account_id: (10_000 + suffix) as u64,
        account_type: AccountType::Patient,
        user_profile_id: (20_000 + suffix) as u64,
        user_main_profile_id: (20_000 + suffix) as u64,
        tenant_id: 1,
        oidc_user_id: Some(format!("test-patient-{suffix}")),
        legacy_data: None,
    }
}

fn reserve_request(suffix: i32) -> ReserveRequest {
    let start = jiff::Timestamp::now().as_second() + 30 * 24 * 60 * 60 + i64::from(suffix % 86_400);

    ReserveRequest {
        doctor_id: 30_000 + suffix,
        biz_unit_id: 1,
        biz_center_id: 1,
        patient_intake: PatientPrescreen {
            symptom: "test symptom".to_string(),
            symptom_duration: 1,
            symptom_duration_unit: "day".to_string(),
            attachments: vec![],
            allergies: vec![],
        },
        consultation_channel: ConsultationChannel::Video,
        timeslot: Timeslot {
            start,
            end: start + 900,
            duration: 900,
        },
        booking_type: BookingType::Instant,
        trace_id: Some(format!("test-trace-{suffix}")),
    }
}

async fn create_test_reservation(pool: &PgPool) -> Result<(String, ReserveRequest), anyhow::Error> {
    let suffix = unique_suffix();
    let user = test_user(suffix);
    let request = reserve_request(suffix);
    let doctor_id = Uuid::new_v4();
    let doctor_account_id = i64::from(40_000 + suffix);
    let doctor_profile_id = i64::from(50_000 + suffix);
    sqlx::query("INSERT INTO v2.doctor_identity (doctor_id, doctor_account_id, doctor_profile_id, is_active) VALUES ($1, $2, $3, true)")
        .bind(doctor_id)
        .bind(doctor_account_id)
        .bind(doctor_profile_id)
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO v2.doctor_consultation_config (doctor_id, instant_available, schedule_available) VALUES ($1, true, false)")
        .bind(doctor_id)
        .execute(pool)
        .await?;
    sqlx::query("INSERT INTO v2.doctor_service_config_projection (doctor_id, channels, languages, duration_minutes, doctor_fee_amount, doctor_fee_currency, profile_version, source_event_id) VALUES ($1, ARRAY['video'], ARRAY['en'], 15, 0, 'THB', 1, $2)")
        .bind(doctor_id)
        .bind(format!("booking-outbox-{suffix}"))
        .execute(pool)
        .await?;
    let repo = AppointmentHoldPsql::new(pool.clone(), TEST_TOPIC);

    let booking_id = repo
        .create_hold(
            &user,
            &CreateAppointmentHold::from(request.clone()),
            doctor_account_id,
            doctor_profile_id,
            900,
            jiff::Timestamp::now().as_second(),
        )
        .await?
        .booking_id;

    Ok((booking_id, request))
}

async fn cleanup_booking(pool: &PgPool, booking_id: &str) {
    let _ = sqlx::query("DELETE FROM v2.event_outbox WHERE aggregate_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM v2.doctor_occupancy WHERE hold_id IN (SELECT hold_id FROM v2.appointment_hold WHERE booking_id = $1) OR appointment_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM v2.appointment WHERE appointment_id = $1 OR booking_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await;
    let _ = sqlx::query("DELETE FROM v2.appointment_hold WHERE booking_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await;
}

async fn outbox_count(
    pool: &PgPool,
    booking_id: &str,
    event_type: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM v2.event_outbox
        WHERE aggregate_id = $1
          AND event_type = $2
        "#,
    )
    .bind(booking_id)
    .bind(event_type)
    .fetch_one(pool)
    .await
}

#[tokio::test]
async fn hold_creation_persists_canonical_hold_occupancy_and_timeslot_reserved_outbox_atomically()
-> Result<(), anyhow::Error> {
    let database = common::setup_test_db().await;
    let pool = database.pool.clone();

    let (booking_id, request) = create_test_reservation(&pool).await?;

    let result = async {
        let (hold_status, topic, payload): (String, String, Value) = sqlx::query_as(
            r#"
            SELECT h.hold_status::text, eo.topic, eo.payload
            FROM v2.appointment_hold h
            JOIN v2.doctor_occupancy o ON o.hold_id = h.hold_id
            JOIN v2.event_outbox eo ON eo.aggregate_id = h.booking_id
            WHERE h.booking_id = $1
              AND o.occupancy_status = 'ACTIVE'::v2.doctor_occupancy_status_enum
              AND eo.event_type = 'TimeslotReserved'
            "#,
        )
        .bind(&booking_id)
        .fetch_one(&pool)
        .await?;

        assert_eq!(hold_status, "ACTIVE");
        assert_eq!(topic, TEST_TOPIC);
        assert_eq!(payload["__type"], "TimeslotReserved");
        assert_eq!(payload["bookingId"], booking_id);
        assert_eq!(payload["doctorId"], request.doctor_id);
        assert_eq!(payload["reservedFrom"], request.timeslot.start);
        assert_eq!(payload["reservationDurationSec"], request.timeslot.duration);
        assert_eq!(
            outbox_count(&pool, &booking_id, "TimeslotReserved").await?,
            1
        );

        Ok::<(), anyhow::Error>(())
    }
    .await;

    cleanup_booking(&pool, &booking_id).await;
    result
}

#[tokio::test]
async fn hold_creation_rolls_back_hold_and_occupancy_when_timeslot_reserved_outbox_insert_fails()
-> Result<(), anyhow::Error> {
    let database = common::setup_test_db().await;
    let pool = database.pool.clone();
    let holds_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM v2.appointment_hold")
        .fetch_one(&pool)
        .await?;
    let occupancy_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM v2.doctor_occupancy")
        .fetch_one(&pool)
        .await?;

    sqlx::query(
        r#"
        CREATE OR REPLACE FUNCTION v2.fail_timeslot_reserved_outbox_for_test()
        RETURNS trigger AS $$
        BEGIN
            IF NEW.event_type = 'TimeslotReserved' THEN
                RAISE EXCEPTION 'test-only TimeslotReserved outbox failure';
            END IF;
            RETURN NEW;
        END;
        $$ LANGUAGE plpgsql;
        "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query("DROP TRIGGER IF EXISTS fail_timeslot_reserved_outbox_for_test ON v2.event_outbox")
        .execute(&pool)
        .await?;
    sqlx::query(
        r#"
        CREATE TRIGGER fail_timeslot_reserved_outbox_for_test
        BEFORE INSERT ON v2.event_outbox
        FOR EACH ROW EXECUTE FUNCTION v2.fail_timeslot_reserved_outbox_for_test()
        "#,
    )
    .execute(&pool)
    .await?;

    let creation = create_test_reservation(&pool).await;

    sqlx::query("DROP TRIGGER IF EXISTS fail_timeslot_reserved_outbox_for_test ON v2.event_outbox")
        .execute(&pool)
        .await?;
    sqlx::query("DROP FUNCTION IF EXISTS v2.fail_timeslot_reserved_outbox_for_test()")
        .execute(&pool)
        .await?;

    assert!(
        creation.is_err(),
        "the injected outbox error must fail creation"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM v2.appointment_hold")
            .fetch_one(&pool)
            .await?,
        holds_before,
        "the failed transaction must not leave a Hold"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM v2.doctor_occupancy")
            .fetch_one(&pool)
            .await?,
        occupancy_before,
        "the failed transaction must not leave Occupancy"
    );
    Ok(())
}

#[tokio::test]
async fn release_persists_released_hold_and_reservation_cancelled_outbox_atomically()
-> Result<(), anyhow::Error> {
    let database = common::setup_test_db().await;
    let pool = database.pool.clone();

    let (booking_id, _) = create_test_reservation(&pool).await?;
    let booking_repo = BookingRepoPsql::new(pool.clone(), TEST_TOPIC);

    let result = async {
        let cancelled = booking_repo
            .cancel_reserved_booking(&booking_id)
            .await?
            .expect("reserved booking should exist");
        assert!(cancelled.state_changed);
        assert_eq!(cancelled.reservation_status, "CANCELLED");

        let (hold_status, topic, payload): (String, String, Value) = sqlx::query_as(
            r#"
            SELECT h.hold_status::text, eo.topic, eo.payload
            FROM v2.appointment_hold h
            JOIN v2.event_outbox eo ON eo.aggregate_id = h.booking_id
            WHERE h.booking_id = $1
              AND eo.event_type = 'ReservationCancelled'
            "#,
        )
        .bind(&booking_id)
        .fetch_one(&pool)
        .await?;

        assert_eq!(hold_status, "RELEASED");
        assert_eq!(topic, TEST_TOPIC);
        assert_eq!(payload["__type"], "ReservationCancelled");
        assert_eq!(payload["bookingId"], booking_id);
        assert_eq!(payload["cancelledAt"], cancelled.cancelled_at);
        assert_eq!(
            outbox_count(&pool, &booking_id, "ReservationCancelled").await?,
            1
        );

        let second_cancel = booking_repo
            .cancel_reserved_booking(&booking_id)
            .await?
            .expect("cancelled booking should still be readable");
        assert!(!second_cancel.state_changed);
        assert_eq!(
            outbox_count(&pool, &booking_id, "ReservationCancelled").await?,
            1
        );

        Ok::<(), anyhow::Error>(())
    }
    .await;

    cleanup_booking(&pool, &booking_id).await;
    result
}

#[tokio::test]
async fn cancel_rolls_back_state_when_outbox_insert_is_rejected_by_lifecycle_unique_guard()
-> Result<(), anyhow::Error> {
    let database = common::setup_test_db().await;
    let pool = database.pool.clone();

    let unique_index_exists: bool = sqlx::query_scalar(
        "SELECT to_regclass('v2.idx_event_outbox_booking_lifecycle_unique') IS NOT NULL",
    )
    .fetch_one(&pool)
    .await?;
    assert!(
        unique_index_exists,
        "fresh migrations must install lifecycle uniqueness"
    );

    let booking_id = short_booking_id("L");
    cleanup_booking(&pool, &booking_id).await;

    sqlx::query(
        r#"
        INSERT INTO v2.appointment_hold (
            booking_id, idempotency_key,
            patient_account_id,
            patient_profile_id,
            doctor_id,
            doctor_account_id,
            doctor_profile_id,
            biz_unit_id,
            biz_center_id,
            tenant_id,
            hold_status, expires_at, purpose_code,
            booking_type,
            consultation_channel,
            starts_at,
            ends_at
        ) VALUES (
            $1, 'test:' || $1,
            10001,
            20001,
            30001,
            40001,
            50001,
            1,
            1,
            1,
            'ACTIVE'::v2.appointment_hold_status_enum,
            NOW() + INTERVAL '15 minutes',
            'PATIENT_BOOKING',
            'Schedule'::v2.booking_type_enum,
            'video'::v2.consultation_type_enum,
            NOW() + INTERVAL '30 days',
            NOW() + INTERVAL '30 days 30 minutes'
        )
        "#,
    )
    .bind(&booking_id)
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO v2.event_outbox (
            event_id,
            topic,
            event_type,
            aggregate_id,
            payload,
            publication_status
        ) VALUES ($1, $2, 'ReservationCancelled', $3, $4, 'PENDING')
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(TEST_TOPIC)
    .bind(&booking_id)
    .bind(serde_json::json!({
        "__type": "ReservationCancelled",
        "bookingId": booking_id,
    }))
    .execute(&pool)
    .await?;

    let booking_repo = BookingRepoPsql::new(pool.clone(), TEST_TOPIC);
    let result = async {
        let cancel_result = booking_repo.cancel_reserved_booking(&booking_id).await;
        assert!(
            cancel_result.is_err(),
            "duplicate lifecycle outbox row should reject cancel transaction"
        );

        let hold_status: String = sqlx::query_scalar(
            "SELECT hold_status::text FROM v2.appointment_hold WHERE booking_id = $1",
        )
        .bind(&booking_id)
        .fetch_one(&pool)
        .await?;

        assert_eq!(hold_status, "ACTIVE");
        assert_eq!(
            outbox_count(&pool, &booking_id, "ReservationCancelled").await?,
            1
        );

        Ok::<(), anyhow::Error>(())
    }
    .await;

    cleanup_booking(&pool, &booking_id).await;
    result
}
