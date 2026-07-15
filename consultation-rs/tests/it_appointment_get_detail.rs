mod common;

use std::sync::Arc;

use consultation_rs::appointment::get_detail::{
    model::GetAppointmentDetailResponse,
    repo::GetAppointmentDetailRepoPsql,
    service::{GetAppointmentDetailError, GetAppointmentDetailService},
};
use consultation_rs::repo::enums::{
    AppointmentStatusEnum, BookingTypeEnum, ConsultationChannelEnum,
};

#[tokio::test]
#[ignore]
async fn test_get_detail_with_raw_json_prescreen() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let booking_id = "BKDETAIL001";

    // Clean up any prior data
    sqlx::query("DELETE FROM v2.appointment_payment_transaction WHERE appointment_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.patient_prescreen WHERE booking_id = $1")
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

    // Insert reservation
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
            tenant_id,
            reservation_status,
            reserved_until,
            booking_type,
            consultation_channel,
            appointment_start,
            appointment_end
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

    // Insert appointment
    // prescreen_data_id is intentionally a phantom value (99999) — the v2.appointment
    // table's FK to patient_prescreen is commented out (see migration 20260222000002),
    // and v2.get_appointment_detail joins via booking_id so this column isn't read
    // back. If the FK is ever re-enabled, change this to capture the prescreen_id
    // from the patient_prescreen INSERT above.
    sqlx::query(
        r#"
        INSERT INTO v2.appointment (
            appointment_id,
            booking_id,
            prescreen_data_id,
            appointment_status,
            appointment_start,
            appointment_end,
            consult_duration,
            has_follow_up
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

    // Insert patient_prescreen
    sqlx::query(
        r#"
        INSERT INTO v2.patient_prescreen (
            booking_id,
            prescreen_data,
            prescreen_data_type,
            user_account_id,
            user_profile_id
        ) VALUES (
            $1,
            '{"symptom":"headache","duration":7,"durationUnit":"day","attachments":["att-1","att-2"],"allergies":["Amoxicillin"]}',
            'RAW_JSON',
            10001,
            20001
        )
        "#,
    )
    .bind(booking_id)
    .execute(pool)
    .await
    .expect("Failed to insert patient_prescreen");

    // Insert appointment_payment_transaction
    sqlx::query(
        r#"
        INSERT INTO v2.appointment_payment_transaction (
            appointment_id,
            payment_tx_id,
            payment_tx_ref_id,
            payment_channels
        ) VALUES (
            $1, 12345, 'tx-ref-detail-001', NULL
        )
        "#,
    )
    .bind(booking_id)
    .execute(pool)
    .await
    .expect("Failed to insert appointment_payment_transaction");

    // Build the service with the real Psql repo
    let service = GetAppointmentDetailService::new(
        Arc::new(GetAppointmentDetailRepoPsql::new(pool.clone())),
        Arc::new(common::MockKms),
        "test-key".to_string(),
    );

    // Call the service
    let response = service
        .get_appointment_detail(booking_id)
        .await
        .expect("query should succeed");

    // Assert on the response
    let detail = match response {
        GetAppointmentDetailResponse::Success(d) => d,
        GetAppointmentDetailResponse::AppointmentNotFound => {
            panic!("Expected Success, got AppointmentNotFound")
        }
    };

    assert_eq!(detail.booking_id, "BKDETAIL001");

    // Epoch seconds for 2026-02-27T09:30:00Z and 2026-02-27T09:45:00Z
    assert_eq!(detail.appointment_time.start_time, 1772184600);
    assert_eq!(detail.appointment_time.end_time, 1772185500);

    assert!(matches!(detail.status, AppointmentStatusEnum::Booked));
    assert!(matches!(detail.booking_type, BookingTypeEnum::Schedule));
    assert!(matches!(
        detail.consultation_channel,
        ConsultationChannelEnum::Video
    ));

    assert_eq!(detail.patient.account_id, 10001);
    assert_eq!(detail.patient.profile_id, 20001);
    assert_eq!(detail.doctor.account_id, 30001);
    assert_eq!(detail.doctor.profile_id, 40001);

    assert_eq!(detail.prescreen.symptom, "headache");
    assert_eq!(detail.prescreen.duration, 7);
    assert_eq!(detail.prescreen.duration_unit, "day");
    assert_eq!(detail.prescreen.attachments, vec!["att-1", "att-2"]);
    assert_eq!(detail.prescreen.allergies, vec!["Amoxicillin"]);

    assert_eq!(detail.payment_tx_id, 12345);
    assert_eq!(detail.payment_tx_ref_id, "tx-ref-detail-001");

    // Post-test cleanup (reverse FK order) so successive runs against the same
    // testcontainer don't accumulate data. Mirrors it_summarization.rs.
    sqlx::query("DELETE FROM v2.appointment_payment_transaction WHERE appointment_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.patient_prescreen WHERE booking_id = $1")
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

    println!("✓ test_get_detail_with_raw_json_prescreen passed");
}

#[tokio::test]
#[ignore]
async fn test_get_detail_returns_appointment_not_found() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let booking_id = "BKNOTFOUND_999";

    // Pre-cleanup to remove any junk from previous runs
    sqlx::query("DELETE FROM v2.appointment_payment_transaction WHERE appointment_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.patient_prescreen WHERE booking_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
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

    // Build the service with the real Psql repo
    let service = GetAppointmentDetailService::new(
        Arc::new(GetAppointmentDetailRepoPsql::new(pool.clone())),
        Arc::new(common::MockKms),
        "test-key".to_string(),
    );

    // Call the service — no row exists for this booking_id
    let response = service
        .get_appointment_detail(booking_id)
        .await
        .expect("query should succeed");

    // Assert the result is AppointmentNotFound
    assert!(
        matches!(response, GetAppointmentDetailResponse::AppointmentNotFound),
        "expected AppointmentNotFound, got Success"
    );

    // No post-test cleanup needed — we didn't insert anything

    println!("✓ test_get_detail_returns_appointment_not_found passed");
}

#[tokio::test]
#[ignore]
async fn test_get_detail_unknown_prescreen_type_errors() {
    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let booking_id = "BKWEIRD_001";

    // Pre-cleanup to remove any junk from previous runs
    sqlx::query("DELETE FROM v2.appointment_payment_transaction WHERE appointment_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.patient_prescreen WHERE booking_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
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

    // Insert reservation
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
            tenant_id,
            reservation_status,
            reserved_until,
            booking_type,
            consultation_channel,
            appointment_start,
            appointment_end
        ) VALUES (
            $1, 10002, 20002, 99, 30002, 40002, 1, 1,
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

    // Insert appointment
    sqlx::query(
        r#"
        INSERT INTO v2.appointment (
            appointment_id,
            booking_id,
            prescreen_data_id,
            appointment_status,
            appointment_start,
            appointment_end,
            consult_duration,
            has_follow_up
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

    // Insert patient_prescreen with an unknown prescreen_data_type
    sqlx::query(
        r#"
        INSERT INTO v2.patient_prescreen (
            booking_id,
            prescreen_data,
            prescreen_data_type,
            user_account_id,
            user_profile_id
        ) VALUES (
            $1,
            '{}',
            'WEIRD_FORMAT',
            10002,
            20002
        )
        "#,
    )
    .bind(booking_id)
    .execute(pool)
    .await
    .expect("Failed to insert patient_prescreen");

    // Payment transaction row is scaffolding for the full appointment row set;
    // the test never reads it because the service errors on prescreen decode
    // before it touches payment data. Included to keep the row set complete
    // and the cleanup symmetric with the other tests.
    sqlx::query(
        r#"
        INSERT INTO v2.appointment_payment_transaction (
            appointment_id,
            payment_tx_id,
            payment_tx_ref_id,
            payment_channels
        ) VALUES (
            $1, 22222, 'tx-ref-weird-001', NULL
        )
        "#,
    )
    .bind(booking_id)
    .execute(pool)
    .await
    .expect("Failed to insert appointment_payment_transaction");

    // Build the service with the real Psql repo
    let service = GetAppointmentDetailService::new(
        Arc::new(GetAppointmentDetailRepoPsql::new(pool.clone())),
        Arc::new(common::MockKms),
        "test-key".to_string(),
    );

    // Call the service — should error because 'WEIRD_FORMAT' is not a known type
    let result = service.get_appointment_detail(booking_id).await;

    // Assert the result is UnsupportedPrescreenDataType("WEIRD_FORMAT")
    match result {
        Err(GetAppointmentDetailError::UnsupportedPrescreenDataType(s)) => {
            assert_eq!(s, "WEIRD_FORMAT");
        }
        other => panic!(
            "expected UnsupportedPrescreenDataType(\"WEIRD_FORMAT\"), got {:?}",
            other
        ),
    }

    // Post-test cleanup (reverse FK order)
    sqlx::query("DELETE FROM v2.appointment_payment_transaction WHERE appointment_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.patient_prescreen WHERE booking_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
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

    println!("✓ test_get_detail_unknown_prescreen_type_errors passed");
}

#[tokio::test]
#[ignore]
async fn test_get_detail_with_enc_gcp_kms_prescreen() {
    use base64::Engine;
    use consultation_rs::appointment::get_detail::repo::GetAppointmentDetailRepoPsql;
    use consultation_rs::appointment::get_detail::service::GetAppointmentDetailService;
    use consultation_rs::sys::crypto::kms::{GcpKmsService, Kms};

    let test_db = common::setup_test_db().await;
    let pool = &test_db.pool;

    let booking_id = "BKKMS_001";

    let kms: Arc<dyn Kms> = Arc::new(
        GcpKmsService::new()
            .await
            .expect("Failed to create GcpKmsService — set GOOGLE_APPLICATION_CREDENTIALS to a service account JSON"),
    );

    let key_name = std::env::var("TEST_KMS_PRESCREEN_KEY").unwrap_or_else(|_| {
        // Fall back to the doctor-note key since we know it exists.
        // Override TEST_KMS_PRESCREEN_KEY to point at the prescreen key once provisioned.
        "projects/tdg-dh-truehealth-core-nonprod/locations/asia-southeast1/keyRings/doctor-note-key/cryptoKeys/doctor-note-key".to_string()
    });

    let prescreen_json = r#"{"symptom":"chest pain","duration":2,"durationUnit":"hour","attachments":["scan-001"],"allergies":["sulfa"]}"#;

    // 1. Encrypt using the shared Arc (clone so we can also pass it to the service)
    let ciphertext = kms
        .encrypt(prescreen_json.as_bytes(), &key_name)
        .await
        .expect("KMS encrypt failed");
    let encoded = base64::engine::general_purpose::STANDARD.encode(&ciphertext);

    // 2. Pre-cleanup
    sqlx::query("DELETE FROM v2.appointment_payment_transaction WHERE appointment_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.patient_prescreen WHERE booking_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
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

    // 3. Insert reservation
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
            tenant_id,
            reservation_status,
            reserved_until,
            booking_type,
            consultation_channel,
            appointment_start,
            appointment_end
        ) VALUES (
            $1, 10003, 20003, 99, 30003, 40003, 1, 1,
            'CONFIRMED'::v2.reservation_status_enum,
            '2026-03-15T10:45:00Z'::timestamptz,
            'Schedule'::v2.booking_type_enum,
            'video'::v2.consultation_type_enum,
            '2026-03-15T10:30:00Z'::timestamptz,
            '2026-03-15T10:45:00Z'::timestamptz
        )
        "#,
    )
    .bind(booking_id)
    .execute(pool)
    .await
    .expect("Failed to insert reservation");

    // 4. Insert appointment
    sqlx::query(
        r#"
        INSERT INTO v2.appointment (
            appointment_id,
            booking_id,
            prescreen_data_id,
            appointment_status,
            appointment_start,
            appointment_end,
            consult_duration,
            has_follow_up
        ) VALUES (
            $1, $1, 99999,
            'BOOKED'::v2.fhir_appointment_status_enum,
            '2026-03-15T10:30:00Z'::timestamptz,
            '2026-03-15T10:45:00Z'::timestamptz,
            '15 minutes'::interval,
            false
        )
        "#,
    )
    .bind(booking_id)
    .execute(pool)
    .await
    .expect("Failed to insert appointment");

    // 5. Insert patient_prescreen with KMS-encrypted data
    sqlx::query(
        r#"
        INSERT INTO v2.patient_prescreen (
            booking_id,
            prescreen_data,
            prescreen_data_type,
            user_account_id,
            user_profile_id
        ) VALUES (
            $1, $2, 'ENC_GCP_KMS', 10003, 20003
        )
        "#,
    )
    .bind(booking_id)
    .bind(&encoded)
    .execute(pool)
    .await
    .expect("Failed to insert patient_prescreen");

    // 6. Insert appointment_payment_transaction
    sqlx::query(
        r#"
        INSERT INTO v2.appointment_payment_transaction (
            appointment_id,
            payment_tx_id,
            payment_tx_ref_id,
            payment_channels
        ) VALUES (
            $1, 33333, 'tx-ref-kms-001', NULL
        )
        "#,
    )
    .bind(booking_id)
    .execute(pool)
    .await
    .expect("Failed to insert appointment_payment_transaction");

    // 7. Build the service with the REAL GcpKmsService (reuse the Arc constructed above)
    let service = GetAppointmentDetailService::new(
        Arc::new(GetAppointmentDetailRepoPsql::new(pool.clone())),
        Arc::clone(&kms),
        key_name.clone(),
    );

    // 8. Call the service
    let response = service
        .get_appointment_detail(booking_id)
        .await
        .expect("query should succeed");

    // 9. Assert round-trip
    let detail = match response {
        GetAppointmentDetailResponse::Success(d) => d,
        GetAppointmentDetailResponse::AppointmentNotFound => {
            panic!("Expected Success, got AppointmentNotFound")
        }
    };

    assert_eq!(detail.booking_id, "BKKMS_001");
    assert_eq!(detail.prescreen.symptom, "chest pain");
    assert_eq!(detail.prescreen.duration, 2);
    assert_eq!(detail.prescreen.duration_unit, "hour");
    assert_eq!(detail.prescreen.attachments, vec!["scan-001"]);
    assert_eq!(detail.prescreen.allergies, vec!["sulfa"]);

    assert_eq!(detail.patient.account_id, 10003);
    assert_eq!(detail.patient.profile_id, 20003);
    assert_eq!(detail.doctor.account_id, 30003);
    assert_eq!(detail.doctor.profile_id, 40003);

    assert_eq!(detail.payment_tx_id, 33333);
    assert_eq!(detail.payment_tx_ref_id, "tx-ref-kms-001");

    // 10. Post-test cleanup (reverse FK order)
    sqlx::query("DELETE FROM v2.appointment_payment_transaction WHERE appointment_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM v2.patient_prescreen WHERE booking_id = $1")
        .bind(booking_id)
        .execute(pool)
        .await
        .ok();
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

    println!("✓ test_get_detail_with_enc_gcp_kms_prescreen passed");
}
