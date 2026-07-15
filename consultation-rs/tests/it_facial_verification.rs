use common_rs::tdh_protocol::iam::user_identity::{AccountType, UserIdentity};
use sqlx::{PgPool, Postgres};

mod common;

use consultation_rs::consultation::session_info::repo::{
    GetOrCreateSessionRepoPsql, SessionManagementRepo,
};
use consultation_rs::repo::enums::ConsultationChannelEnum;

// Helper struct for facial upload operations
struct FacialUploadHelper {
    pool: sqlx::Pool<Postgres>,
}

impl FacialUploadHelper {
    fn new(pool: sqlx::Pool<Postgres>) -> Self {
        Self { pool }
    }

    async fn insert_facial_upload(
        &self,
        appointment_id: &str,
        user_profile_id: i32,
        user_account_id: i32,
        object_url: &str,
    ) -> Result<(), anyhow::Error> {
        sqlx::query(
            r#"
            INSERT INTO v2.appointment_facial_upload (
                appointment_id, user_profile_id, user_account_id, object_url
            ) VALUES ($1, $2, $3, $4)
            ON CONFLICT (appointment_id) DO UPDATE SET
                object_url = EXCLUDED.object_url,
                created_at = NOW()
            "#,
        )
        .bind(appointment_id)
        .bind(user_profile_id)
        .bind(user_account_id)
        .bind(object_url)
        .execute(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to insert facial upload: {}", e))?;

        Ok(())
    }
}

/// Helper function to insert test reservation and appointment
async fn setup_test_appointment(
    pool: &PgPool,
    booking_id: &str,
    channel: ConsultationChannelEnum,
    patient_profile_id: i32,
    doctor_profile_id: i32,
) -> Result<(), anyhow::Error> {
    let channel_str = match channel {
        ConsultationChannelEnum::Video => "video",
        ConsultationChannelEnum::Voice => "voice",
        ConsultationChannelEnum::Chat => "chat",
    };

    // Insert reservation
    sqlx::query(
        r#"
        INSERT INTO v2.reservation (
            booking_id, patient_account_id, patient_profile_id,
            doctor_id, doctor_account_id, doctor_profile_id,
            biz_unit_id, biz_center_id, tenant_id,
            reservation_status, reserved_until, booking_type, consultation_channel,
            appointment_start, appointment_end
        ) VALUES ($1, $2, $3, $4, $5, $6, 1, 100, 1, 'CONFIRMED',
                  now() + INTERVAL '2 hour', 'Instant',
                  $7::v2.consultation_type_enum,
                  now() + INTERVAL '1 hour', now() + INTERVAL '2 hour')
        ON CONFLICT (booking_id) DO NOTHING
        "#,
    )
    .bind(booking_id)
    .bind(patient_profile_id) // patient_account_id
    .bind(patient_profile_id)
    .bind(doctor_profile_id) // doctor_id
    .bind(doctor_profile_id) // doctor_account_id
    .bind(doctor_profile_id)
    .bind(channel_str)
    .execute(pool)
    .await?;

    // Insert appointment
    sqlx::query(
        r#"
        INSERT INTO v2.appointment (
            appointment_id, booking_id, prescreen_data_id, appointment_status,
            appointment_start, consult_duration, appointment_end, has_follow_up
        ) VALUES ($1, $1, 0, 'BOOKED', now() + INTERVAL '1 hour', INTERVAL '1 hour',
                  now() + INTERVAL '2 hour', FALSE)
        ON CONFLICT (appointment_id) DO NOTHING
        "#,
    )
    .bind(booking_id)
    .execute(pool)
    .await?;

    Ok(())
}

/// Helper function to clean up test data
async fn cleanup_test_data(pool: &PgPool, booking_id: &str) -> Result<(), anyhow::Error> {
    sqlx::query("DELETE FROM v2.appointment_facial_upload WHERE appointment_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await?;

    sqlx::query("DELETE FROM v2.session_info WHERE appointment_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await?;

    sqlx::query("DELETE FROM v2.appointment WHERE appointment_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await?;

    sqlx::query("DELETE FROM v2.reservation WHERE booking_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_facial_verification_flow_video_channel() {
    // Setup
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let session_repo = GetOrCreateSessionRepoPsql::new(pool.clone());
    let facial_helper = FacialUploadHelper::new(pool.clone());

    let booking_id = "FV001";
    let patient_profile_id = 8888i32;
    let doctor_profile_id = 5555i32;

    // Clean up any existing test data
    cleanup_test_data(pool, booking_id).await.ok();

    // Setup test appointment with VIDEO channel
    setup_test_appointment(
        pool,
        booking_id,
        ConsultationChannelEnum::Video,
        patient_profile_id,
        doctor_profile_id,
    )
    .await
    .expect("Failed to setup test appointment");

    let user_identity = UserIdentity {
        account_id: 8888u64,
        account_type: AccountType::Patient,
        user_profile_id: patient_profile_id as u64,
        user_main_profile_id: patient_profile_id as u64,
        tenant_id: 1,
        oidc_user_id: Some("test-fv-user".to_string()),
        legacy_data: None,
    };

    // TEST 1: Before facial upload - is_facial_verified should be false
    let session = session_repo
        .get_appointment_session(&user_identity, booking_id)
        .await
        .expect("Failed to get appointment session")
        .expect("Session should exist");

    assert!(
        matches!(session.consultation_channel, ConsultationChannelEnum::Video),
        "Channel should be Video"
    );
    assert!(
        !session.is_facial_verified,
        "is_facial_verified should be false before upload"
    );
    println!("✓ TEST 1: is_facial_verified = false (before upload)");

    // TEST 2: Simulate facial upload by directly inserting into DB (stub GCS upload)
    // In real flow, this would be called after successful GCS upload
    facial_helper
        .insert_facial_upload(
            booking_id,
            patient_profile_id,
            8888,                                            // account_id
            "gs://test-bucket/facial/FV001_patient_success", // stub URL
        )
        .await
        .expect("Failed to insert facial upload record");

    // TEST 3: After facial upload - is_facial_verified should be true
    let session = session_repo
        .get_appointment_session(&user_identity, booking_id)
        .await
        .expect("Failed to get appointment session")
        .expect("Session should exist");

    assert!(
        session.is_facial_verified,
        "is_facial_verified should be true after upload"
    );
    println!("✓ TEST 2: is_facial_verified = true (after upload)");

    // TEST 4: Doctor should also see the verification status
    let doctor_identity = UserIdentity {
        account_id: 5555u64,
        account_type: AccountType::Doctor,
        user_profile_id: doctor_profile_id as u64,
        user_main_profile_id: doctor_profile_id as u64,
        tenant_id: 1,
        oidc_user_id: Some("test-fv-doctor".to_string()),
        legacy_data: None,
    };

    let session = session_repo
        .get_appointment_session(&doctor_identity, booking_id)
        .await
        .expect("Failed to get appointment session")
        .expect("Session should exist");

    assert!(
        session.is_facial_verified,
        "Doctor should see is_facial_verified = true"
    );
    println!("✓ TEST 3: Doctor can see is_facial_verified = true");

    // Cleanup
    cleanup_test_data(pool, booking_id).await.ok();
}

#[tokio::test]
#[ignore]
async fn test_facial_verification_voice_channel_always_true() {
    // Setup
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let session_repo = GetOrCreateSessionRepoPsql::new(pool.clone());
    let facial_helper = FacialUploadHelper::new(pool.clone());

    let booking_id = "FV002";
    let patient_profile_id = 8889i32;
    let doctor_profile_id = 5556i32;

    cleanup_test_data(pool, booking_id).await.ok();

    // Setup test appointment with VOICE channel
    setup_test_appointment(
        pool,
        booking_id,
        ConsultationChannelEnum::Voice,
        patient_profile_id,
        doctor_profile_id,
    )
    .await
    .expect("Failed to setup test appointment");

    let user_identity = UserIdentity {
        account_id: 8889u64,
        account_type: AccountType::Patient,
        user_profile_id: patient_profile_id as u64,
        user_main_profile_id: patient_profile_id as u64,
        tenant_id: 1,
        oidc_user_id: Some("test-fv-user2".to_string()),
        legacy_data: None,
    };

    // TEST: Non-VIDEO channels should always return true
    let session = session_repo
        .get_appointment_session(&user_identity, booking_id)
        .await
        .expect("Failed to get appointment session")
        .expect("Session should exist");

    assert!(
        matches!(session.consultation_channel, ConsultationChannelEnum::Voice),
        "Channel should be Voice"
    );
    assert!(
        session.is_facial_verified,
        "is_facial_verified should be true for non-VIDEO channels"
    );
    println!("✓ TEST: is_facial_verified = true for VOICE channel (no upload needed)");

    // Even inserting a facial upload record should still return true
    facial_helper
        .insert_facial_upload(
            booking_id,
            patient_profile_id,
            8889,
            "gs://test-bucket/facial/FV002_patient_success",
        )
        .await
        .ok();

    let session = session_repo
        .get_appointment_session(&user_identity, booking_id)
        .await
        .expect("Failed to get appointment session")
        .expect("Session should exist");

    assert!(
        session.is_facial_verified,
        "is_facial_verified should still be true for VOICE channel"
    );

    // Cleanup
    cleanup_test_data(pool, booking_id).await.ok();
}

#[tokio::test]
#[ignore]
async fn test_facial_verification_multiple_uploads() {
    // Setup
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let session_repo = GetOrCreateSessionRepoPsql::new(pool.clone());
    let facial_helper = FacialUploadHelper::new(pool.clone());

    let booking_id = "FV003";
    let patient_profile_id = 8890i32;
    let doctor_profile_id = 5557i32;

    cleanup_test_data(pool, booking_id).await.ok();

    setup_test_appointment(
        pool,
        booking_id,
        ConsultationChannelEnum::Video,
        patient_profile_id,
        doctor_profile_id,
    )
    .await
    .expect("Failed to setup test appointment");

    let user_identity = UserIdentity {
        account_id: 8890u64,
        account_type: AccountType::Patient,
        user_profile_id: patient_profile_id as u64,
        user_main_profile_id: patient_profile_id as u64,
        tenant_id: 1,
        oidc_user_id: Some("test-fv-user3".to_string()),
        legacy_data: None,
    };

    // First upload
    facial_helper
        .insert_facial_upload(
            booking_id,
            patient_profile_id,
            8890,
            "gs://test-bucket/facial/FV003_patient_v1",
        )
        .await
        .expect("Failed to insert first facial upload");

    let session = session_repo
        .get_appointment_session(&user_identity, booking_id)
        .await
        .expect("Failed to get appointment session")
        .expect("Session should exist");

    assert!(
        session.is_facial_verified,
        "is_facial_verified should be true after first upload"
    );
    println!("✓ TEST 1: is_facial_verified = true after first upload");

    // Second upload (update) - ON CONFLICT should update the record
    facial_helper
        .insert_facial_upload(
            booking_id,
            patient_profile_id,
            8890,
            "gs://test-bucket/facial/FV003_patient_v2",
        )
        .await
        .expect("Failed to update facial upload");

    let session = session_repo
        .get_appointment_session(&user_identity, booking_id)
        .await
        .expect("Failed to get appointment session")
        .expect("Session should exist");

    assert!(
        session.is_facial_verified,
        "is_facial_verified should still be true after update"
    );
    println!("✓ TEST 2: is_facial_verified = true after update");

    // Verify the URL was updated in DB
    let (url,): (String,) = sqlx::query_as(
        "SELECT object_url FROM v2.appointment_facial_upload WHERE appointment_id = $1",
    )
    .bind(booking_id)
    .fetch_one(pool)
    .await
    .expect("Failed to fetch object_url");

    assert!(url.contains("_v2"), "Object URL should be updated to v2");
    println!("✓ TEST 3: Object URL was updated correctly");

    // Cleanup
    cleanup_test_data(pool, booking_id).await.ok();
}
