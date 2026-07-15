mod common;

use consultation_rs::sys::crypto::kms::{GcpKmsService, Kms};

fn kms_key_name() -> String {
    std::env::var("TEST_KMS_DOCTOR_NOTE_KEY").unwrap_or_else(|_| {
        "projects/tdg-dh-truehealth-core-nonprod/locations/asia-southeast1/keyRings/doctor-note-key/cryptoKeys/doctor-note-key".to_string()
    })
}

#[tokio::test]
#[ignore]
async fn test_gcp_kms_encrypt_decrypt_roundtrip() {
    common::init_test_logging();

    let kms = GcpKmsService::new()
        .await
        .expect("Failed to create GcpKmsService — check GOOGLE_APPLICATION_CREDENTIALS");

    let key_name = kms_key_name();
    let plaintext = b"hello from consultation summarization integration test";

    let ciphertext = kms
        .encrypt(plaintext, &key_name)
        .await
        .expect("KMS encrypt failed");

    assert!(!ciphertext.is_empty(), "Ciphertext should not be empty");
    assert_ne!(
        ciphertext.as_slice(),
        plaintext.as_slice(),
        "Ciphertext should differ from plaintext"
    );

    let decrypted = kms
        .decrypt(&ciphertext, &key_name)
        .await
        .expect("KMS decrypt failed");

    assert_eq!(
        decrypted, plaintext,
        "Decrypted data should match original plaintext"
    );
}

#[tokio::test]
#[ignore]
async fn test_gcp_kms_encrypt_produces_different_ciphertexts() {
    common::init_test_logging();

    let kms = GcpKmsService::new()
        .await
        .expect("Failed to create GcpKmsService");

    let key_name = kms_key_name();
    let plaintext = b"same input data";

    let ct1 = kms
        .encrypt(plaintext, &key_name)
        .await
        .expect("First encrypt failed");
    let ct2 = kms
        .encrypt(plaintext, &key_name)
        .await
        .expect("Second encrypt failed");

    assert_ne!(
        ct1, ct2,
        "KMS should produce different ciphertexts for the same plaintext (random IV)"
    );

    let dec1 = kms
        .decrypt(&ct1, &key_name)
        .await
        .expect("Decrypt 1 failed");
    let dec2 = kms
        .decrypt(&ct2, &key_name)
        .await
        .expect("Decrypt 2 failed");

    assert_eq!(dec1.as_slice(), plaintext);
    assert_eq!(dec2.as_slice(), plaintext);
}

#[tokio::test]
#[ignore]
async fn test_gcp_kms_encrypt_large_payload() {
    common::init_test_logging();

    let kms = GcpKmsService::new()
        .await
        .expect("Failed to create GcpKmsService");

    let key_name = kms_key_name();
    let large_payload = vec![0xAB_u8; 32 * 1024];

    let ciphertext = kms
        .encrypt(&large_payload, &key_name)
        .await
        .expect("Encrypt of 32KB payload failed");

    let decrypted = kms
        .decrypt(&ciphertext, &key_name)
        .await
        .expect("Decrypt of 32KB payload failed");

    assert_eq!(
        decrypted, large_payload,
        "Large payload roundtrip should match"
    );
}

#[tokio::test]
#[ignore]
async fn test_gcp_kms_encrypt_json_serialized_summary_note() {
    use consultation_rs::protocol::summary_note::{DurationUnit, Icd10, SummarizationRequest};

    common::init_test_logging();

    let kms = GcpKmsService::new()
        .await
        .expect("Failed to create GcpKmsService");

    let key_name = kms_key_name();

    let note = SummarizationRequest {
        booking_id: 12344.to_string(),
        prescription_id: Some(67890),
        present_illness: "Fever and cough for 3 days".to_string(),
        chief_complaint: "Fever".to_string(),
        diagnosis: "Upper respiratory infection".to_string(),
        recommendations: "Rest and fluids".to_string(),
        icd10: vec![Icd10 {
            code: "J00".to_string(),
            description: "Acute nasopharyngitis".to_string(),
        }],
        illness_duration: DurationUnit {
            value: 3,
            unit: "days".to_string(),
        },
        note_to_staff: "Allergic to penicillin".to_string(),
        follow_up: consultation_rs::protocol::follow_up::FollowUp::AsNeeded,
        drug_allergies: None,
    };

    let json_data = serde_json::to_vec(&note).expect("Failed to serialize SummarizationRequest");

    let ciphertext = kms
        .encrypt(&json_data, &key_name)
        .await
        .expect("Encrypt of SummarizationRequest JSON failed");

    let decrypted = kms
        .decrypt(&ciphertext, &key_name)
        .await
        .expect("Decrypt of SummarizationRequest JSON failed");

    assert_eq!(
        decrypted, json_data,
        "Decrypted SummarizationRequest JSON should match original"
    );

    let restored: SummarizationRequest = serde_json::from_slice(&decrypted)
        .expect("Failed to deserialize decrypted SummarizationRequest");

    assert_eq!(restored.booking_id, 12345.to_string());
    assert_eq!(restored.icd10.len(), 1);
    assert_eq!(restored.icd10[0].code, "J00");
}
