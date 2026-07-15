mod common;

use common_rs::tdh_protocol::appointment::v2::payment_transaction::PaymentChannel;
use common_rs::tdh_protocol::common::PartialUserIdentity;
use common_rs::tdh_protocol::consultation::consultation_pre_screen::ConsultationPreScreen;
use common_rs::tdh_protocol::consultation::{BookingType, ConsultationChannel};
use common_rs::tdh_protocol::internal::CreateConfirmedInstantAppointmentRequest;
use consultation_rs::internal::repo::InternalRepo;
use consultation_rs::internal::service::CreateConfirmedAppointment;

#[tokio::test]
#[ignore]
async fn test_create_confirmed_appointment_instant() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    common::cleanup_test_data(pool).await;
    common::seed_test_data(pool).await;

    let repo = InternalRepo::new(pool.clone());
    let service = CreateConfirmedAppointment::new(std::sync::Arc::new(repo));

    let request = CreateConfirmedInstantAppointmentRequest {
        biz_unit_id: 1,
        biz_center_id: 100,
        tenant_id: 1,
        patient_id: PartialUserIdentity {
            account_id: 5001,
            user_profile_id: 6001,
            tenant_id: 1,
            oidc_user_id: Some("test-patient-1".to_string()),
        },
        doctor_id: PartialUserIdentity {
            account_id: 7001,
            user_profile_id: 8001,
            tenant_id: 1,
            oidc_user_id: Some("test-doctor-1".to_string()),
        },
        prescreen: ConsultationPreScreen {
            symptom: "Test symptom".to_string(),
            duration: 3,
            duration_unit: "days".to_string(),
            attachments: vec![],
            allergies: vec![],
        },
        consult_duration: Some(20),
        booking_type: BookingType::Instant,
        consultation_channel: ConsultationChannel::Video,
        parent_appointment_id: None,
        payment_channels: vec![PaymentChannel::Card {
            id: "124".to_string(),
        }],
    };

    let result = service.create_confirmed_appointment(request).await;

    assert!(
        result.is_ok(),
        "Should successfully create confirmed appointment: {:?}",
        result.err()
    );

    let booking_id = result.unwrap();
    assert!(booking_id > 0, "Booking ID should be positive");
    let booking_id_str = booking_id.to_string();

    let reservation_check: Option<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT
            booking_id,
            reservation_status::text,
            booking_type::text
        FROM v2.reservation
        WHERE booking_id = $1
        "#,
    )
    .bind(&booking_id_str)
    .fetch_optional(pool)
    .await
    .expect("Failed to query reservation");

    assert!(
        reservation_check.is_some(),
        "Reservation should exist in database for booking_id {}",
        booking_id
    );
    let (res_booking_id, res_status, res_booking_type) = reservation_check.unwrap();
    assert_eq!(res_booking_id, booking_id_str, "Booking ID should match");
    assert_eq!(
        res_status, "CONFIRMED",
        "Reservation status should be CONFIRMED"
    );
    assert_eq!(
        res_booking_type, "Instant",
        "Booking type should be Instant"
    );

    let appointment_check: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT
            appointment_id,
            appointment_status::text
        FROM v2.appointment
        WHERE appointment_id = $1
        "#,
    )
    .bind(&booking_id_str)
    .fetch_optional(pool)
    .await
    .expect("Failed to query appointment");

    assert!(
        appointment_check.is_some(),
        "Appointment should exist in database"
    );
    let (apt_id, apt_status) = appointment_check.unwrap();
    assert_eq!(apt_id, booking_id_str, "Appointment ID should match");
    assert_eq!(apt_status, "BOOKED", "Appointment status should be BOOKED");

    println!(
        "✓ Successfully created instant appointment with booking_id: {}",
        booking_id
    );

    cleanup_created_appointment(pool, &booking_id_str).await;
    common::cleanup_test_data(pool).await;
}

#[tokio::test]
#[ignore]
async fn test_create_confirmed_appointment_with_custom_duration() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    common::cleanup_test_data(pool).await;
    common::seed_test_data(pool).await;

    let repo = InternalRepo::new(pool.clone());
    let service = CreateConfirmedAppointment::new(std::sync::Arc::new(repo));

    let request = CreateConfirmedInstantAppointmentRequest {
        biz_unit_id: 1,
        biz_center_id: 100,
        tenant_id: 1,
        patient_id: PartialUserIdentity {
            account_id: 5002,
            user_profile_id: 6002,
            tenant_id: 1,
            oidc_user_id: Some("test-patient-2".to_string()),
        },
        doctor_id: PartialUserIdentity {
            account_id: 7002,
            user_profile_id: 8002,
            tenant_id: 1,
            oidc_user_id: Some("test-doctor-2".to_string()),
        },
        prescreen: ConsultationPreScreen {
            symptom: "Test symptom".to_string(),
            duration: 3,
            duration_unit: "days".to_string(),
            attachments: vec![],
            allergies: vec![],
        },
        consult_duration: Some(30),
        booking_type: BookingType::Instant,
        consultation_channel: ConsultationChannel::Voice,
        parent_appointment_id: None,
        payment_channels: vec![PaymentChannel::Card {
            id: "124".to_string(),
        }],
    };

    let result = service.create_confirmed_appointment(request).await;

    assert!(result.is_ok(), "Should create appointment successfully");
    let booking_id = result.unwrap();
    let booking_id_str = booking_id.to_string();

    let time_check: Option<(i64, i64)> = sqlx::query_as(
        r#"
        SELECT
            EXTRACT(EPOCH FROM appointment_start)::bigint as start_time,
            EXTRACT(EPOCH FROM appointment_end)::bigint as end_time
        FROM v2.appointment
        WHERE appointment_id = $1
        "#,
    )
    .bind(&booking_id_str)
    .fetch_optional(pool)
    .await
    .expect("Failed to query appointment times");

    assert!(time_check.is_some(), "Appointment times should exist");
    let (start_time, end_time) = time_check.unwrap();
    let duration_minutes = (end_time - start_time) / 60;

    assert!(
        (duration_minutes - 30).abs() <= 1,
        "Duration should be approximately 30 minutes, got {} minutes",
        duration_minutes
    );

    println!(
        "✓ Created appointment with custom 30-minute duration: booking_id {}",
        booking_id
    );

    cleanup_created_appointment(pool, &booking_id_str).await;
    common::cleanup_test_data(pool).await;
}

#[tokio::test]
#[ignore]
async fn test_create_confirmed_appointment_chat_channel() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    common::cleanup_test_data(pool).await;
    common::seed_test_data(pool).await;

    let repo = InternalRepo::new(pool.clone());
    let service = CreateConfirmedAppointment::new(std::sync::Arc::new(repo));

    let request = CreateConfirmedInstantAppointmentRequest {
        biz_unit_id: 2,
        biz_center_id: 101,
        tenant_id: 1,
        patient_id: PartialUserIdentity {
            account_id: 5003,
            user_profile_id: 6003,
            tenant_id: 1,
            oidc_user_id: Some("test-patient-3".to_string()),
        },
        doctor_id: PartialUserIdentity {
            account_id: 7003,
            user_profile_id: 8003,
            tenant_id: 1,
            oidc_user_id: Some("test-doctor-3".to_string()),
        },
        prescreen: ConsultationPreScreen {
            symptom: "Test symptom".to_string(),
            duration: 3,
            duration_unit: "days".to_string(),
            attachments: vec![],
            allergies: vec![],
        },
        consult_duration: Some(20),
        booking_type: BookingType::Instant,
        consultation_channel: ConsultationChannel::Chat,
        parent_appointment_id: None,
        payment_channels: vec![PaymentChannel::Card {
            id: "124".to_string(),
        }],
    };

    let result = service.create_confirmed_appointment(request).await;

    assert!(
        result.is_ok(),
        "Should create chat appointment successfully"
    );
    let booking_id = result.unwrap();
    let booking_id_str = booking_id.to_string();

    let channel_check: Option<(String, i32)> = sqlx::query_as(
        r#"
        SELECT
            consultation_channel::text,
            biz_unit_id
        FROM v2.reservation
        WHERE booking_id = $1
        "#,
    )
    .bind(&booking_id_str)
    .fetch_optional(pool)
    .await
    .expect("Failed to query consultation channel");

    assert!(channel_check.is_some(), "Channel data should exist");
    let (channel, biz_unit) = channel_check.unwrap();
    assert_eq!(channel, "chat", "Channel should be chat");
    assert_eq!(biz_unit, 2, "Biz unit should be 2 (Insurance)");

    println!("✓ Created chat appointment: booking_id {}", booking_id);

    cleanup_created_appointment(pool, &booking_id_str).await;
    common::cleanup_test_data(pool).await;
}

#[tokio::test]
#[ignore]
async fn test_create_confirmed_appointment_with_follow_up() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    common::cleanup_test_data(pool).await;
    common::seed_test_data(pool).await;

    let repo = InternalRepo::new(pool.clone());
    let service = CreateConfirmedAppointment::new(std::sync::Arc::new(repo));

    let parent_request = CreateConfirmedInstantAppointmentRequest {
        biz_unit_id: 1,
        biz_center_id: 100,
        tenant_id: 1,
        patient_id: PartialUserIdentity {
            account_id: 5004,
            user_profile_id: 6004,
            tenant_id: 1,
            oidc_user_id: Some("test-patient-4".to_string()),
        },
        doctor_id: PartialUserIdentity {
            account_id: 7004,
            user_profile_id: 8004,
            tenant_id: 1,
            oidc_user_id: Some("test-doctor-4".to_string()),
        },
        prescreen: ConsultationPreScreen {
            symptom: "Test symptom".to_string(),
            duration: 3,
            duration_unit: "days".to_string(),
            attachments: vec![],
            allergies: vec![],
        },
        consult_duration: Some(20),
        booking_type: BookingType::Instant,
        consultation_channel: ConsultationChannel::Video,
        parent_appointment_id: None,
        payment_channels: vec![PaymentChannel::Card {
            id: "124".to_string(),
        }],
    };

    let parent_booking_id = service
        .create_confirmed_appointment(parent_request)
        .await
        .expect("Failed to create parent appointment");
    let parent_booking_id_str = parent_booking_id.to_string();

    let followup_request = CreateConfirmedInstantAppointmentRequest {
        biz_unit_id: 1,
        biz_center_id: 100,
        tenant_id: 1,
        patient_id: PartialUserIdentity {
            account_id: 5004,
            user_profile_id: 6004,
            tenant_id: 1,
            oidc_user_id: Some("test-patient-4".to_string()),
        },
        doctor_id: PartialUserIdentity {
            account_id: 7004,
            user_profile_id: 8004,
            tenant_id: 1,
            oidc_user_id: Some("test-doctor-4".to_string()),
        },
        prescreen: ConsultationPreScreen {
            symptom: "Test symptom".to_string(),
            duration: 3,
            duration_unit: "days".to_string(),
            attachments: vec![],
            allergies: vec![],
        },
        consult_duration: Some(20),
        booking_type: BookingType::Instant,
        consultation_channel: ConsultationChannel::Video,
        parent_appointment_id: Some(parent_booking_id_str.clone()),
        payment_channels: vec![PaymentChannel::Card {
            id: "124".to_string(),
        }],
    };

    let result = service.create_confirmed_appointment(followup_request).await;

    assert!(
        result.is_ok(),
        "Should create follow-up appointment successfully"
    );
    let followup_booking_id = result.unwrap();
    let followup_booking_id_str = followup_booking_id.to_string();

    let parent_check: Option<(Option<String>,)> = sqlx::query_as(
        r#"
        SELECT parent_appointment_id::text
        FROM v2.appointment
        WHERE appointment_id = $1
        "#,
    )
    .bind(&followup_booking_id_str)
    .fetch_optional(pool)
    .await
    .expect("Failed to query parent appointment");

    assert!(parent_check.is_some(), "Follow-up appointment should exist");
    let (parent_id,) = parent_check.unwrap();
    assert_eq!(
        parent_id,
        Some(parent_booking_id_str.clone()),
        "Parent appointment ID should match"
    );

    println!(
        "✓ Created follow-up appointment {} for parent {}",
        followup_booking_id, parent_booking_id
    );

    cleanup_created_appointment(pool, &followup_booking_id_str).await;
    cleanup_created_appointment(pool, &parent_booking_id_str).await;
    common::cleanup_test_data(pool).await;
}

#[tokio::test]
#[ignore]
async fn test_create_confirmed_appointment_unique_booking_id() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    common::cleanup_test_data(pool).await;
    common::seed_test_data(pool).await;

    let repo = InternalRepo::new(pool.clone());
    let service = CreateConfirmedAppointment::new(std::sync::Arc::new(repo));

    let mut booking_ids = Vec::new();

    for i in 0..5 {
        let request = CreateConfirmedInstantAppointmentRequest {
            biz_unit_id: 1,
            biz_center_id: 100,
            tenant_id: 1,
            patient_id: PartialUserIdentity {
                account_id: 5100 + i,
                user_profile_id: 6100 + i,
                tenant_id: 1,
                oidc_user_id: Some(format!("test-patient-{}", i)),
            },
            doctor_id: PartialUserIdentity {
                account_id: 7100 + i,
                user_profile_id: 8100 + i,
                tenant_id: 1,
                oidc_user_id: Some(format!("test-doctor-{}", i)),
            },
            prescreen: ConsultationPreScreen {
                symptom: "Test symptom".to_string(),
                duration: 3,
                duration_unit: "days".to_string(),
                attachments: vec![],
                allergies: vec![],
            },
            consult_duration: Some(20),
            booking_type: BookingType::Instant,
            consultation_channel: ConsultationChannel::Video,
            parent_appointment_id: None,
            payment_channels: vec![PaymentChannel::Card {
                id: "124".to_string(),
            }],
        };

        let booking_id = service
            .create_confirmed_appointment(request)
            .await
            .expect("Failed to create appointment");

        booking_ids.push(booking_id);
    }

    let unique_count = booking_ids
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert_eq!(
        unique_count,
        booking_ids.len(),
        "All booking IDs should be unique"
    );

    let today = jiff::Timestamp::now().strftime("%Y%m%d").to_string();
    for booking_id in &booking_ids {
        let id_str = booking_id.to_string();
        assert!(
            id_str.starts_with(&today),
            "Booking ID {} should start with date prefix {}",
            booking_id,
            today
        );
    }

    println!(
        "✓ Created {} appointments with unique booking IDs",
        booking_ids.len()
    );

    for booking_id in booking_ids {
        cleanup_created_appointment(pool, &booking_id.to_string()).await;
    }
    common::cleanup_test_data(pool).await;
}

#[tokio::test]
#[ignore]
async fn test_create_confirmed_appointment_default_duration() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    common::cleanup_test_data(pool).await;
    common::seed_test_data(pool).await;

    let repo = InternalRepo::new(pool.clone());
    let service = CreateConfirmedAppointment::new(std::sync::Arc::new(repo));

    let request = CreateConfirmedInstantAppointmentRequest {
        biz_unit_id: 1,
        biz_center_id: 100,
        tenant_id: 1,
        patient_id: PartialUserIdentity {
            account_id: 5200,
            user_profile_id: 6200,
            tenant_id: 1,
            oidc_user_id: Some("test-patient-default".to_string()),
        },
        doctor_id: PartialUserIdentity {
            account_id: 7200,
            user_profile_id: 8200,
            tenant_id: 1,
            oidc_user_id: Some("test-doctor-default".to_string()),
        },
        prescreen: ConsultationPreScreen {
            symptom: "Test symptom".to_string(),
            duration: 3,
            duration_unit: "days".to_string(),
            attachments: vec![],
            allergies: vec![],
        },
        consult_duration: None,
        booking_type: BookingType::Instant,
        consultation_channel: ConsultationChannel::Video,
        parent_appointment_id: None,
        payment_channels: vec![PaymentChannel::Card {
            id: "124".to_string(),
        }],
    };

    let result = service.create_confirmed_appointment(request).await;

    assert!(
        result.is_ok(),
        "Should create appointment with default duration"
    );
    let booking_id = result.unwrap();
    let booking_id_str = booking_id.to_string();

    let time_check: Option<(i64, i64)> = sqlx::query_as(
        r#"
        SELECT
            EXTRACT(EPOCH FROM appointment_start)::bigint as start_time,
            EXTRACT(EPOCH FROM appointment_end)::bigint as end_time
        FROM v2.appointment
        WHERE appointment_id = $1
        "#,
    )
    .bind(&booking_id_str)
    .fetch_optional(pool)
    .await
    .expect("Failed to query appointment times");

    assert!(time_check.is_some(), "Appointment times should exist");
    let (start_time, end_time) = time_check.unwrap();
    let duration_minutes = (end_time - start_time) / 60;

    assert!(
        (duration_minutes - 20).abs() <= 1,
        "Default duration should be approximately 20 minutes, got {} minutes",
        duration_minutes
    );

    println!(
        "✓ Verified default duration of 20 minutes for booking_id {}",
        booking_id
    );

    cleanup_created_appointment(pool, &booking_id_str).await;
    common::cleanup_test_data(pool).await;
}

async fn cleanup_created_appointment(pool: &sqlx::PgPool, booking_id: &str) {
    sqlx::query("DELETE FROM v2.appointment WHERE appointment_id = $1")
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
