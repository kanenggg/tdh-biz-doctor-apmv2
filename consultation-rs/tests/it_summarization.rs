mod common;

use std::sync::Arc;

use common_rs::tdh_protocol::iam::user_identity::{AccountType, UserIdentity};
use consultation_rs::infra::event::NoOpEventPublisher;
use consultation_rs::protocol::follow_up::FollowUp;
use consultation_rs::protocol::summary_note::{DurationUnit, Icd10, SummarizationRequest};
use consultation_rs::summarization::SummarizationResult;
use consultation_rs::summarization::follow_up_repo::FollowUpRepoPsql;
use consultation_rs::summarization::repo::{CreateSummaryNoteParams, SummaryNoteRepoPsql};
use consultation_rs::summarization::service::SummaryNoteService;

fn create_test_summary_note(booking_id: String) -> SummarizationRequest {
    SummarizationRequest {
        booking_id,
        prescription_id: Some(12345),
        present_illness: "Patient has fever and cough".to_string(),
        chief_complaint: "Fever for 3 days".to_string(),
        diagnosis: "Upper respiratory infection".to_string(),
        recommendations: "Rest and drink plenty of fluids".to_string(),
        icd10: vec![
            Icd10 {
                code: "J00".to_string(),
                description: "Acute nasopharyngitis".to_string(),
            },
            Icd10 {
                code: "R50".to_string(),
                description: "Fever".to_string(),
            },
        ],
        illness_duration: DurationUnit {
            value: 3,
            unit: "days".to_string(),
        },
        note_to_staff: "Patient allergic to penicillin".to_string(),
        follow_up: FollowUp::AsNeeded,
        drug_allergies: None,
    }
}

#[tokio::test]
#[ignore]
async fn test_create_summary_note_idempotent() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let test_appointment_id = "99991";

    sqlx::query("DELETE FROM v2.doctor_summary_note WHERE appointment_id = $1")
        .bind(test_appointment_id)
        .execute(pool)
        .await
        .ok();

    let repo = SummaryNoteRepoPsql::new(pool.clone());

    let params = CreateSummaryNoteParams {
        booking_id: test_appointment_id.to_string(),
        encrypted_data: "encrypted_test_data_1".to_string(),
        encrypted_data_type: "DoctorSummarizationRequestV1".to_string(),
        note_to_staff: Some("Test note".to_string()),
        icd10_codes: vec!["J00".to_string(), "R50".to_string()],
        prescription_id: Some(12345),
    };

    let result1 = repo.insert(params.clone()).await;
    assert!(
        result1.is_ok(),
        "First insert should succeed: {:?}",
        result1.err()
    );
    assert!(
        result1.unwrap().created,
        "First insert should return created=true"
    );

    let result2 = repo.insert(params).await;
    assert!(
        result2.is_ok(),
        "Second insert should succeed: {:?}",
        result2.err()
    );
    assert!(
        !result2.unwrap().created,
        "Second insert should return created=false (already existed)"
    );

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM v2.doctor_summary_note WHERE appointment_id = $1")
            .bind(test_appointment_id)
            .fetch_one(pool)
            .await
            .expect("Failed to count records");

    assert_eq!(
        count, 1,
        "Should have exactly 1 record after idempotent inserts"
    );

    sqlx::query("DELETE FROM v2.doctor_summary_note WHERE appointment_id = $1")
        .bind(test_appointment_id)
        .execute(pool)
        .await
        .ok();

    println!("✓ Summary note idempotent test passed");
}

#[tokio::test]
#[ignore]
async fn test_create_summary_note_different_appointments() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let appointment_ids = ["99992", "99993", "99994"];

    for id in &appointment_ids {
        sqlx::query("DELETE FROM v2.doctor_summary_note WHERE appointment_id = $1")
            .bind(id)
            .execute(pool)
            .await
            .ok();
    }

    let repo = SummaryNoteRepoPsql::new(pool.clone());

    for (i, appointment_id) in appointment_ids.iter().enumerate() {
        let params = CreateSummaryNoteParams {
            booking_id: appointment_id.to_string(),
            encrypted_data: format!("encrypted_data_{}", i),
            encrypted_data_type: "DoctorSummarizationRequestV1".to_string(),
            note_to_staff: Some(format!("Note for appointment {}", appointment_id)),
            icd10_codes: vec![format!("CODE{}", i)],
            prescription_id: Some(10000 + i as i64),
        };

        let result = repo.insert(params).await;
        assert!(
            result.is_ok(),
            "Insert should succeed for {}: {:?}",
            appointment_id,
            result.err()
        );
        assert!(
            result.unwrap().created,
            "Insert should return created=true for {}",
            appointment_id
        );
    }

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM v2.doctor_summary_note WHERE appointment_id = ANY($1)",
    )
    .bind(&appointment_ids[..])
    .fetch_one(pool)
    .await
    .expect("Failed to count records");

    assert_eq!(count, 3, "Should have 3 records for different appointments");

    for id in &appointment_ids {
        sqlx::query("DELETE FROM v2.doctor_summary_note WHERE appointment_id = $1")
            .bind(id)
            .execute(pool)
            .await
            .ok();
    }

    println!("✓ Summary note different appointments test passed");
}

#[tokio::test]
#[ignore]
async fn test_create_summary_note_with_null_prescription() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let test_appointment_id = "99995";

    sqlx::query("DELETE FROM v2.doctor_summary_note WHERE appointment_id = $1")
        .bind(test_appointment_id)
        .execute(pool)
        .await
        .ok();

    let repo = SummaryNoteRepoPsql::new(pool.clone());

    let params = CreateSummaryNoteParams {
        booking_id: test_appointment_id.to_string(),
        encrypted_data: "encrypted_data_no_prescription".to_string(),
        encrypted_data_type: "DoctorSummarizationRequestV1".to_string(),
        note_to_staff: None,
        icd10_codes: vec![],
        prescription_id: None,
    };

    let result = repo.insert(params).await;
    assert!(
        result.is_ok(),
        "Insert with null prescription should succeed: {:?}",
        result.err()
    );
    assert!(result.unwrap().created, "Insert should return created=true");

    let record: (Option<String>, Option<i64>) = sqlx::query_as(
        "SELECT note_to_staff, prescription_id FROM v2.doctor_summary_note WHERE appointment_id = $1",
    )
    .bind(test_appointment_id)
    .fetch_one(pool)
    .await
    .expect("Failed to fetch record");

    assert!(record.0.is_none(), "Note to staff should be null");
    assert!(record.1.is_none(), "Prescription ID should be null");

    sqlx::query("DELETE FROM v2.doctor_summary_note WHERE appointment_id = $1")
        .bind(test_appointment_id)
        .execute(pool)
        .await
        .ok();

    println!("✓ Summary note with null prescription test passed");
}

#[tokio::test]
#[ignore]
async fn test_summary_note_service_idempotent() {
    use base64::Engine;

    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let test_appointment_id = "99996i64".to_string();

    sqlx::query("DELETE FROM v2.doctor_summary_note WHERE appointment_id = $1")
        .bind(test_appointment_id.clone())
        .execute(pool)
        .await
        .ok();

    let repo = Arc::new(SummaryNoteRepoPsql::new(pool.clone()));
    let follow_up_repo = Arc::new(FollowUpRepoPsql::new(pool.clone()));
    let kms = Arc::new(common::MockKms);
    let event_publisher = Arc::new(NoOpEventPublisher);
    let service = SummaryNoteService::new(
        repo,
        follow_up_repo,
        event_publisher,
        kms,
        "test-key".to_string(),
    );

    let user_identity = UserIdentity {
        account_id: 1001,
        account_type: AccountType::Doctor,
        user_profile_id: 2001,
        user_main_profile_id: 2001,
        tenant_id: 1,
        oidc_user_id: Some("test-doctor".to_string()),
        legacy_data: None,
    };

    let summary_note1 = create_test_summary_note(test_appointment_id.clone());
    let summary_note2 = create_test_summary_note(test_appointment_id.clone());

    let result1 = service
        .add_summary_note(Some(user_identity.clone()), summary_note1)
        .await;
    assert!(
        result1.is_ok(),
        "First service call should succeed: {:?}",
        result1.err()
    );
    assert!(matches!(
        result1.unwrap(),
        SummarizationResult::Success { .. }
    ));

    let result2 = service
        .add_summary_note(Some(user_identity), summary_note2)
        .await;
    assert!(
        result2.is_ok(),
        "Second service call should succeed: {:?}",
        result2.err()
    );
    assert!(matches!(
        result2.unwrap(),
        SummarizationResult::AlreadySubmitted { .. }
    ));

    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM v2.doctor_summary_note WHERE appointment_id = $1")
            .bind(test_appointment_id.to_string())
            .fetch_one(pool)
            .await
            .expect("Failed to count records");

    assert_eq!(
        count, 1,
        "Should have exactly 1 record after idempotent service calls"
    );

    let encrypted_data: String = sqlx::query_scalar(
        "SELECT encrypted_data FROM v2.doctor_summary_note WHERE appointment_id = $1",
    )
    .bind(test_appointment_id.to_string())
    .fetch_one(pool)
    .await
    .expect("Failed to fetch encrypted data");

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(&encrypted_data)
        .expect("Failed to decode base64");
    let stored_note: SummarizationRequest =
        serde_json::from_slice(&decoded).expect("Failed to deserialize stored note");

    assert_eq!(stored_note.booking_id, test_appointment_id);
    assert_eq!(stored_note.prescription_id, Some(12345));
    assert_eq!(stored_note.icd10.len(), 2);
    assert_eq!(stored_note.icd10[0].code, "J00");

    sqlx::query("DELETE FROM v2.doctor_summary_note WHERE appointment_id = $1")
        .bind(test_appointment_id.to_string())
        .execute(pool)
        .await
        .ok();

    println!("✓ Summary note service idempotent test passed");
}

#[tokio::test]
#[ignore]
async fn test_submit_summary_fulfills_appointment() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let booking_id = "BKFULFIL001";

    // Clean up any prior data.
    sqlx::query("DELETE FROM v2.doctor_summary_note WHERE appointment_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
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

    // Seed a reservation + a BOOKED appointment (the pre-summary state).
    sqlx::query(
        r#"
        INSERT INTO v2.reservation (
            booking_id, patient_account_id, patient_profile_id,
            doctor_id, doctor_account_id, doctor_profile_id,
            biz_unit_id, tenant_id, reservation_status, reserved_until,
            booking_type, consultation_channel, appointment_start, appointment_end
        ) VALUES (
            $1, 10001, 20001, 99, 30001, 40001, 1, 1,
            'CONFIRMED'::v2.reservation_status_enum,
            '2026-02-27T09:45:00Z'::timestamptz,
            'Schedule'::v2.booking_type_enum,
            'video'::v2.consultation_type_enum,
            '2026-02-27T09:30:00Z'::timestamptz,
            '2026-02-27T09:45:00Z'::timestamptz
        )
        "#,
    )
    .bind(booking_id)
    .execute(pool)
    .await
    .expect("Failed to insert reservation");

    sqlx::query(
        r#"
        INSERT INTO v2.appointment (
            appointment_id, booking_id, prescreen_data_id, appointment_status,
            appointment_start, appointment_end, consult_duration, has_follow_up
        ) VALUES (
            $1, $1, 99999,
            'BOOKED'::v2.fhir_appointment_status_enum,
            '2026-02-27T09:30:00Z'::timestamptz,
            '2026-02-27T09:45:00Z'::timestamptz,
            '15 minutes'::interval,
            false
        )
        "#,
    )
    .bind(booking_id)
    .execute(pool)
    .await
    .expect("Failed to insert appointment");

    // Submit the summary note.
    let repo = SummaryNoteRepoPsql::new(pool.clone());
    let params = CreateSummaryNoteParams {
        booking_id: booking_id.to_string(),
        encrypted_data: "encrypted_test_data".to_string(),
        encrypted_data_type: "DoctorSummaryNoteV1".to_string(),
        note_to_staff: Some("note".to_string()),
        icd10_codes: vec!["J00".to_string()],
        prescription_id: Some(12345),
    };
    let result = repo.insert(params).await;
    assert!(
        result.is_ok(),
        "Summary insert should succeed: {:?}",
        result.err()
    );

    // Submitting the summary must transition the appointment to FULFILLED.
    let status: String = sqlx::query_scalar(
        "SELECT appointment_status::text FROM v2.appointment WHERE appointment_id = $1",
    )
    .bind(booking_id)
    .fetch_one(pool)
    .await
    .expect("Failed to fetch appointment_status");

    assert_eq!(
        status, "FULFILLED",
        "Appointment should be FULFILLED after summary submit"
    );

    // Clean up.
    sqlx::query("DELETE FROM v2.doctor_summary_note WHERE appointment_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
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

    println!("✓ Submit summary fulfils appointment test passed");
}

#[tokio::test]
#[ignore]
async fn test_summary_note_icd10_codes_stored_as_jsonb() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let test_appointment_id = "99997";

    sqlx::query("DELETE FROM v2.doctor_summary_note WHERE appointment_id = $1")
        .bind(test_appointment_id)
        .execute(pool)
        .await
        .ok();

    let repo = SummaryNoteRepoPsql::new(pool.clone());

    let icd10_codes = vec!["J00".to_string(), "R50".to_string(), "Z23".to_string()];

    let params = CreateSummaryNoteParams {
        booking_id: test_appointment_id.to_string(),
        encrypted_data: "test_encrypted".to_string(),
        encrypted_data_type: "DoctorSummarizationRequestV1".to_string(),
        note_to_staff: None,
        icd10_codes: icd10_codes.clone(),
        prescription_id: None,
    };

    let result = repo.insert(params).await;
    assert!(result.is_ok(), "Insert should succeed: {:?}", result.err());

    let stored_codes: sqlx::types::Json<Vec<String>> = sqlx::query_scalar(
        "SELECT icd10_codes FROM v2.doctor_summary_note WHERE appointment_id = $1",
    )
    .bind(test_appointment_id)
    .fetch_one(pool)
    .await
    .expect("Failed to fetch icd10_codes");

    assert_eq!(stored_codes.0, icd10_codes, "ICD10 codes should match");

    sqlx::query("DELETE FROM v2.doctor_summary_note WHERE appointment_id = $1")
        .bind(test_appointment_id)
        .execute(pool)
        .await
        .ok();

    println!("✓ ICD10 codes JSONB storage test passed");
}
