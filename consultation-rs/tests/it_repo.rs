use common_rs::tdh_protocol::appointment::v2::payment_transaction::PaymentChannel;
use common_rs::tdh_protocol::iam::user_identity::{AccountType, UserIdentity};

mod common;

use consultation_rs::consultation::session_info::repo::{
    GetOrCreateSessionRepoPsql, SessionManagementRepo,
};
use consultation_rs::repo::enums::{
    AppointmentStatusEnum, ConsultationChannelEnum, DbMeetingProvider,
};
use consultation_rs::repo::provider_session_info::SessionData;

#[tokio::test]
#[ignore] // Mark as integration test - run with: cargo test --test it_repo -- --ignored
async fn test_get_consultation_session_serde() {
    // Setup
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let repo = GetOrCreateSessionRepoPsql::new(pool.clone());

    // Test user identity (patient from seed data)
    let user_identity = UserIdentity {
        account_id: 1001,
        account_type: AccountType::Patient,
        user_profile_id: 2001,
        user_main_profile_id: 2001,
        tenant_id: 1,
        oidc_user_id: Some("test-user-1".to_string()),
        legacy_data: None,
    };

    // Act - get session for appointment_id 10001
    let result = repo
        .get_appointment_session(&user_identity, "10001")
        .await
        .expect("Failed to get appointment session");

    // Assert
    assert!(result.is_some(), "Expected to find a session");
    let session = result.unwrap();

    // Validate basic fields
    assert_eq!(
        session.appointment_id, "10001",
        "Appointment ID should match"
    );
    assert_eq!(
        session.patient_profile_id, 2001,
        "Patient profile ID should match"
    );
    assert_eq!(
        session.doctor_profile_id, 4001,
        "Doctor profile ID should match"
    );

    // Validate enum deserialization from database
    assert!(
        matches!(session.session_provider_name, DbMeetingProvider::Twilio),
        "Session provider should be Twilio, got: {:?}",
        session.session_provider_name
    );

    assert!(
        matches!(session.appointment_status, AppointmentStatusEnum::Booked),
        "Session status should be EmptyRoomCreated, got: {:?}",
        session.appointment_status
    );

    assert!(
        matches!(session.consultation_channel, ConsultationChannelEnum::Video),
        "Consultation channel should be Video, got: {:?}",
        session.consultation_channel
    );

    let channels = session
        .payment_channels
        .expect("Payment channels should exist")
        .0;
    assert!(
        matches!(&channels[..], [PaymentChannel::Card { id }] if id == "124"),
        "Payment channel should be Card with id=124, got: {:?}",
        channels
    );

    // Validate session_data JSON deserialization
    assert!(session.session_data.is_some(), "Session data should exist");
    let json_value = session.session_data.unwrap();
    let session_data: SessionData =
        serde_json::from_value(json_value.0).expect("Failed to deserialize session data");

    match session_data {
        SessionData::Twilio(twilio_info) => {
            assert!(
                !twilio_info.recording_url.is_empty(),
                "Recording URL should not be empty"
            );
            assert!(
                !twilio_info.session_chat_id.is_empty(),
                "Session chat ID should not be empty"
            );
            println!(
                "✓ Twilio session data deserialized correctly: {:?}",
                twilio_info
            );
        }
        SessionData::TokBox(_) => {
            panic!("Expected Twilio session data, got TokBox");
        }
    }

    // Validate time fields
    assert!(
        session.consultation_start_time > 0,
        "Start time should be set"
    );
    assert!(session.consultation_end_time > 0, "End time should be set");
    assert!(
        session.consultation_end_time > session.consultation_start_time,
        "End time should be after start time"
    );

    // Cleanup
    common::cleanup_test_data(pool).await;
}

#[tokio::test]
#[ignore] // Mark as integration test
async fn test_get_consultation_session_insurance_payment() {
    // Setup
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    // common::cleanup_test_data(pool).await;
    // common::seed_test_data(pool).await;

    let repo = GetOrCreateSessionRepoPsql::new(pool.clone());

    // Test user identity (patient from seed data, booking 10002 has biz_unit_id=2)
    let user_identity = UserIdentity {
        account_id: 1002,
        account_type: AccountType::Patient,
        user_profile_id: 2002,
        user_main_profile_id: 2002,
        tenant_id: 1,
        oidc_user_id: Some("test-user-2".to_string()),
        legacy_data: None,
    };

    // Act - get session for appointment_id 10002 (Insurance payment)
    let result = repo
        .get_appointment_session(&user_identity, "10002")
        .await
        .expect("Failed to get appointment session");

    // Assert
    assert!(result.is_some(), "Expected to find a session");
    let session = result.unwrap();

    assert_eq!(
        session.appointment_id, "10002",
        "Appointment ID should match"
    );

    let channels = session
        .payment_channels
        .expect("Payment channels should exist")
        .0;
    assert!(
        channels
            .iter()
            .any(|c| matches!(c, PaymentChannel::Insurance { .. })),
        "Payment channel should be Insurance (biz_unit_id=2), got: {:?}",
        channels
    );

    assert!(
        matches!(session.consultation_channel, ConsultationChannelEnum::Voice),
        "Consultation channel should be Voice, got: {:?}",
        session.consultation_channel
    );

    assert!(
        matches!(session.appointment_status, AppointmentStatusEnum::Fulfilled),
        "Session status should be Ended, got: {:?}",
        session.appointment_status
    );

    // Cleanup
    common::cleanup_test_data(pool).await;
}

#[tokio::test]
#[ignore] // Mark as integration test
async fn test_get_consultation_session_not_found() {
    // Setup
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    common::cleanup_test_data(pool).await;
    common::seed_test_data(pool).await;

    let repo = GetOrCreateSessionRepoPsql::new(pool.clone());

    let user_identity = UserIdentity {
        account_id: 9999,
        account_type: AccountType::Patient,
        user_profile_id: 9999,
        user_main_profile_id: 9999,
        tenant_id: 1,
        oidc_user_id: Some("test-user-unknown".to_string()),
        legacy_data: None,
    };

    // Act - get session for non-existent appointment
    let result = repo
        .get_appointment_session(&user_identity, "99999")
        .await
        .expect("Query should succeed");

    // Assert
    assert!(
        result.is_none(),
        "Should not find a session for non-existent appointment"
    );

    // Cleanup
    common::cleanup_test_data(pool).await;
}

#[tokio::test]
#[ignore] // Mark as integration test
async fn test_get_consultation_session_unauthorized() {
    // Setup
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    common::cleanup_test_data(pool).await;
    common::seed_test_data(pool).await;

    let repo = GetOrCreateSessionRepoPsql::new(pool.clone());

    // Try to access appointment 10001 with wrong user
    let wrong_user = UserIdentity {
        account_id: 1002,
        account_type: AccountType::Patient,
        user_profile_id: 2002,
        user_main_profile_id: 2002,
        tenant_id: 1,
        oidc_user_id: Some("test-user-2".to_string()),
        legacy_data: None,
    };

    // Act - should not find session because user is not authorized
    let result = repo
        .get_appointment_session(&wrong_user, "10001")
        .await
        .expect("Query should succeed");

    // Assert
    assert!(
        result.is_none(),
        "Should not find session when user is not authorized"
    );

    // Cleanup
    common::cleanup_test_data(pool).await;
}

#[tokio::test]
#[ignore] // Mark as integration test
// Test that session_data can be initialized (repository-level test)
// Note: This test only validates the repo layer, not the full service with Twilio integration
async fn test_init_session_data() {
    // Setup
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    common::cleanup_test_data(pool).await;
    common::seed_test_data(pool).await;

    let repo = GetOrCreateSessionRepoPsql::new(pool.clone());

    // Create new session data
    let new_session_data = SessionData::Twilio(
        consultation_rs::repo::provider_session_info::TwilioSessionInfo {
            recording_url: "https://example.com/new-recording".to_string(),
            session_chat_id: "NEW_CH001".to_string(),
            chat_recording_url: "https://example.com/new-chat-recording".to_string(),
            session_chat_service_id: "NEW_IS001".to_string(),
            session_room_id: None,
            session_room_name: None,
        },
    );

    // Act - update session data for appointment 10001 (first initialization)
    let result = repo.init_session_data("10001", new_session_data).await;

    // Assert
    assert!(
        result.is_ok(),
        "Should successfully update session data: {:?}",
        result.err()
    );

    // Verify the update
    let user_identity = UserIdentity {
        account_id: 1001,
        account_type: AccountType::Patient,
        user_profile_id: 2001,
        user_main_profile_id: 2001,
        tenant_id: 1,
        oidc_user_id: Some("test-user-1".to_string()),
        legacy_data: None,
    };

    let session = repo
        .get_appointment_session(&user_identity, "10001")
        .await
        .expect("Failed to get appointment session")
        .expect("Session should exist");

    // Verify new session data
    assert!(session.session_data.is_some(), "Session data should exist");
    let json_value = session.session_data.unwrap();
    let session_data: SessionData =
        serde_json::from_value(json_value.0).expect("Failed to deserialize session data");
    match session_data {
        SessionData::Twilio(info) => {
            assert_eq!(
                info.session_chat_id, "NEW_CH001",
                "Session chat ID should be updated"
            );
            println!("✓ Session data initialized successfully");
        }
        _ => panic!("Expected Twilio session data"),
    }

    // Cleanup
    common::cleanup_test_data(pool).await;
}

#[tokio::test]
#[ignore] // Mark as integration test
async fn test_serde_json_roundtrip() {
    // Setup
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    common::cleanup_test_data(pool).await;
    common::seed_test_data(pool).await;

    let repo = GetOrCreateSessionRepoPsql::new(pool.clone());

    let user_identity = UserIdentity {
        account_id: 1001,
        account_type: AccountType::Patient,
        user_profile_id: 2001,
        user_main_profile_id: 2001,
        tenant_id: 1,
        oidc_user_id: Some("test-user-1".to_string()),
        legacy_data: None,
    };

    // Get session
    let session = repo
        .get_appointment_session(&user_identity, "10001")
        .await
        .expect("Failed to get appointment session")
        .expect("Session should exist");

    // Serialize to JSON
    let session_json = serde_json::to_string(&session.session_data).expect("Failed to serialize");
    println!("Session data JSON: {}", session_json);

    // Deserialize back
    let deserialized: Option<SessionData> =
        serde_json::from_str(&session_json).expect("Failed to deserialize");

    // Verify roundtrip
    assert!(deserialized.is_some(), "Deserialized data should exist");

    // Deserialize original to SessionData for comparison
    let original_session_data: SessionData = session
        .session_data
        .map(|j| serde_json::from_value(j.0))
        .transpose()
        .expect("Failed to deserialize original session data")
        .expect("Original session data should exist");

    // Compare original and deserialized
    match (original_session_data, deserialized.unwrap()) {
        (SessionData::Twilio(orig), SessionData::Twilio(deser)) => {
            assert_eq!(
                orig.recording_url, deser.recording_url,
                "Recording URL should match"
            );
            assert_eq!(
                orig.session_chat_id, deser.session_chat_id,
                "Chat ID should match"
            );
            println!("✓ Serde JSON roundtrip successful");
        }
        _ => panic!("Session data types don't match after roundtrip"),
    }

    // Cleanup
    common::cleanup_test_data(pool).await;
}

#[tokio::test]
#[ignore] // Mark as integration test
async fn test_is_facial_verified_flag() {
    // Setup
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let repo = GetOrCreateSessionRepoPsql::new(pool.clone());

    // Clean up any existing test data
    sqlx::query("DELETE FROM v2.appointment_facial_upload WHERE appointment_id = '99999'")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.session_info WHERE appointment_id = '99999'")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.appointment WHERE appointment_id = '99999'")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.reservation WHERE booking_id = '99999'")
        .execute(pool)
        .await
        .ok();

    // Insert test reservation with VIDEO channel
    sqlx::query(
        r#"
        INSERT INTO v2.reservation (
            booking_id, patient_account_id, patient_profile_id,
            doctor_id, doctor_account_id, doctor_profile_id,
            biz_unit_id, biz_center_id, tenant_id,
            reservation_status, booking_type, consultation_channel,
            appointment_start, appointment_end, reserved_until
        ) VALUES (
            '99999', 9999, 8888, 7777, 6666, 5555,
            1, 100, 1, 'CONFIRMED', 'Instant', 'video',
            now() + INTERVAL '1 hour', now() + INTERVAL '2 hour',
            now() + INTERVAL '30 minutes'
        )
        "#,
    )
    .execute(pool)
    .await
    .expect("Failed to insert reservation");

    // Insert test appointment
    sqlx::query(
        r#"
        INSERT INTO v2.appointment (
            appointment_id, booking_id, prescreen_data_id, appointment_status,
            appointment_start, consult_duration, appointment_end, has_follow_up
        ) VALUES (
            '99999', '99999', 0, 'BOOKED',
            now() + INTERVAL '1 hour', INTERVAL '1 hour',
            now() + INTERVAL '2 hour', FALSE
        )
        "#,
    )
    .execute(pool)
    .await
    .expect("Failed to insert appointment");

    let user_identity = UserIdentity {
        account_id: 9999,
        account_type: AccountType::Patient,
        user_profile_id: 8888,
        user_main_profile_id: 8888,
        tenant_id: 1,
        oidc_user_id: Some("test-facial-user".to_string()),
        legacy_data: None,
    };

    // TEST 1: First entry - no facial upload yet (should be false for VIDEO)
    let result = repo
        .get_appointment_session(&user_identity, "99999")
        .await
        .expect("Failed to get appointment session");

    assert!(result.is_some(), "Expected to find a session");
    let session = result.unwrap();

    assert!(
        matches!(session.consultation_channel, ConsultationChannelEnum::Video),
        "Consultation channel should be Video"
    );
    assert!(
        !session.is_facial_verified,
        "is_facial_verified should be false when no facial upload exists for VIDEO channel"
    );
    println!("✓ TEST 1: is_facial_verified = false (no facial upload)");

    // TEST 2: After facial upload (should be true for VIDEO)
    sqlx::query(
        r#"
        INSERT INTO v2.appointment_facial_upload (
            appointment_id, user_profile_id, user_account_id, object_url
        ) VALUES ('99999', 8888, 9999, 'https://storage.example.com/facial/99999.jpg')
        "#,
    )
    .execute(pool)
    .await
    .expect("Failed to insert facial upload");

    let result = repo
        .get_appointment_session(&user_identity, "99999")
        .await
        .expect("Failed to get appointment session");

    assert!(result.is_some(), "Expected to find a session");
    let session = result.unwrap();

    assert!(
        session.is_facial_verified,
        "is_facial_verified should be true when facial upload exists for VIDEO channel"
    );
    println!("✓ TEST 2: is_facial_verified = true (after facial upload)");

    // Cleanup
    sqlx::query("DELETE FROM v2.appointment_facial_upload WHERE appointment_id = '99999'")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.session_info WHERE appointment_id = '99999'")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.appointment WHERE appointment_id = '99999'")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.reservation WHERE booking_id = '99999'")
        .execute(pool)
        .await
        .ok();
}

#[tokio::test]
#[ignore] // Mark as integration test
async fn test_is_facial_verified_non_video_channel() {
    // Setup
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let repo = GetOrCreateSessionRepoPsql::new(pool.clone());

    // Clean up
    sqlx::query("DELETE FROM v2.appointment_facial_upload WHERE appointment_id = '99998'")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.session_info WHERE appointment_id = '99998'")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.appointment WHERE appointment_id = '99998'")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.reservation WHERE booking_id = '99998'")
        .execute(pool)
        .await
        .ok();

    // Insert test reservation with VOICE channel
    sqlx::query(
        r#"
        INSERT INTO v2.reservation (
            booking_id, patient_account_id, patient_profile_id,
            doctor_id, doctor_account_id, doctor_profile_id,
            biz_unit_id, biz_center_id, tenant_id,
            reservation_status, booking_type, consultation_channel,
            appointment_start, appointment_end, reserved_until
        ) VALUES (
            '99998', 9999, 8888, 7777, 6666, 5555,
            1, 100, 1, 'CONFIRMED', 'Instant', 'voice',
            now() + INTERVAL '1 hour', now() + INTERVAL '2 hour',
            now() + INTERVAL '30 minutes'
        )
        "#,
    )
    .execute(pool)
    .await
    .expect("Failed to insert reservation");

    sqlx::query(
        r#"
        INSERT INTO v2.appointment (
            appointment_id, booking_id, prescreen_data_id, appointment_status,
            appointment_start, consult_duration, appointment_end, has_follow_up
        ) VALUES (
            '99998', '99998', 0, 'BOOKED',
            now() + INTERVAL '1 hour', INTERVAL '1 hour',
            now() + INTERVAL '2 hour', FALSE
        )
        "#,
    )
    .execute(pool)
    .await
    .expect("Failed to insert appointment");

    let user_identity = UserIdentity {
        account_id: 9999,
        account_type: AccountType::Patient,
        user_profile_id: 8888,
        user_main_profile_id: 8888,
        tenant_id: 1,
        oidc_user_id: Some("test-facial-user".to_string()),
        legacy_data: None,
    };

    // TEST: Non-VIDEO channel should return true
    let result = repo
        .get_appointment_session(&user_identity, "99998")
        .await
        .expect("Failed to get appointment session");

    assert!(result.is_some(), "Expected to find a session");
    let session = result.unwrap();

    assert!(
        matches!(session.consultation_channel, ConsultationChannelEnum::Voice),
        "Consultation channel should be Voice"
    );
    assert!(
        session.is_facial_verified,
        "is_facial_verified should be true for non-VIDEO channels"
    );
    println!("✓ TEST: is_facial_verified = true for non-VIDEO channel");

    // Cleanup
    sqlx::query("DELETE FROM v2.appointment_facial_upload WHERE appointment_id = '99998'")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.session_info WHERE appointment_id = '99998'")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.appointment WHERE appointment_id = '99998'")
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.reservation WHERE booking_id = '99998'")
        .execute(pool)
        .await
        .ok();
}

#[tokio::test]
#[ignore] // Mark as integration test
async fn test_get_consultation_session_first_time_join() {
    // Setup
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;
    common::cleanup_test_data(pool).await;
    common::seed_test_data(pool).await;

    let repo = GetOrCreateSessionRepoPsql::new(pool.clone());

    // Test user identity for booking 10003 (FOLLOW_UP, no session_info in seed data)
    let user_identity = UserIdentity {
        account_id: 1001,
        account_type: AccountType::Patient,
        user_profile_id: 2001,
        user_main_profile_id: 2001,
        tenant_id: 1,
        oidc_user_id: Some("test-user-1".to_string()),
        legacy_data: None,
    };

    // Act - get session for appointment 10003 (first-time join, no session_info exists)
    let result = repo
        .get_appointment_session(&user_identity, "10003")
        .await
        .expect("Failed to get appointment session");

    // Assert
    assert!(
        result.is_some(),
        "Expected to find a session even when session_info doesn't exist"
    );
    let session = result.unwrap();

    // Validate basic fields
    assert_eq!(
        session.appointment_id, "10003",
        "Appointment ID should match"
    );

    // Validate that defaults are provided for missing session_info
    assert!(
        matches!(session.appointment_status, AppointmentStatusEnum::Booked),
        "Session status should default to EmptyRoomCreated when session_info doesn't exist, got: {:?}",
        session.appointment_status
    );

    assert!(
        session.session_data.is_none(),
        "Session data should be None when session_info doesn't exist"
    );

    assert!(
        matches!(session.session_provider_name, DbMeetingProvider::Twilio),
        "Session provider should default to Twilio"
    );

    println!("✓ First-time join handled correctly with defaults");

    // Cleanup
    common::cleanup_test_data(pool).await;
}
