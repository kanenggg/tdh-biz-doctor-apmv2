use std::sync::Arc;

use base64::Engine;

use super::model::{
    AppointmentDetail, GetAppointmentDetailResponse, PartialUserIdentity, PrescreenInfo,
};
use super::repo::GetAppointmentDetailRepo;
use crate::appointment::types::AppointmentTime;
use crate::sys::crypto::kms::Kms;

pub(crate) const PRESCREEN_TYPE_RAW_JSON: &str = "RAW_JSON";
pub(crate) const PRESCREEN_TYPE_ENC_GCP_KMS: &str = "ENC_GCP_KMS";

#[derive(Debug, thiserror::Error)]
pub enum GetAppointmentDetailError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] anyhow::Error),
    #[error("Unsupported prescreen data type: {0}")]
    UnsupportedPrescreenDataType(String),
    #[error("Prescreen base64 decode error: {0}")]
    PrescreenBase64Error(#[from] base64::DecodeError),
    #[error("Prescreen KMS error: {0}")]
    PrescreenKmsError(String),
    #[error("Prescreen UTF-8 decode error: {0}")]
    PrescreenUtf8Error(#[from] std::string::FromUtf8Error),
    #[error("Prescreen JSON parse error: {0}")]
    PrescreenParseError(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct GetAppointmentDetailService {
    repo: Arc<dyn GetAppointmentDetailRepo>,
    kms: Arc<dyn Kms>,
    kms_key_name: String,
}

impl GetAppointmentDetailService {
    pub fn new(
        repo: Arc<dyn GetAppointmentDetailRepo>,
        kms: Arc<dyn Kms>,
        kms_key_name: String,
    ) -> Self {
        Self {
            repo,
            kms,
            kms_key_name,
        }
    }

    pub async fn get_appointment_detail(
        &self,
        booking_id: &str,
    ) -> Result<GetAppointmentDetailResponse, GetAppointmentDetailError> {
        let row = self
            .repo
            .get_appointment_detail(booking_id)
            .await
            .map_err(GetAppointmentDetailError::DatabaseError)?;

        match row {
            None => Ok(GetAppointmentDetailResponse::AppointmentNotFound),
            Some(r) => {
                let prescreen = self
                    .decode_prescreen(&r.prescreen_data_type, &r.prescreen_data)
                    .await?;
                let detail = row_to_detail(r, prescreen);
                Ok(GetAppointmentDetailResponse::Success(detail))
            }
        }
    }

    pub(crate) async fn decode_prescreen(
        &self,
        data_type: &str,
        data: &str,
    ) -> Result<PrescreenInfo, GetAppointmentDetailError> {
        match data_type {
            PRESCREEN_TYPE_RAW_JSON => {
                let info = serde_json::from_str(data)?;
                Ok(info)
            }
            PRESCREEN_TYPE_ENC_GCP_KMS => {
                let ciphertext = base64::engine::general_purpose::STANDARD.decode(data)?;
                let plaintext = self
                    .kms
                    .decrypt(&ciphertext, &self.kms_key_name)
                    .await
                    .map_err(|e| GetAppointmentDetailError::PrescreenKmsError(e.to_string()))?;
                let json = String::from_utf8(plaintext)?;
                let info = serde_json::from_str(&json)?;
                Ok(info)
            }
            other => Err(GetAppointmentDetailError::UnsupportedPrescreenDataType(
                other.to_string(),
            )),
        }
    }
}

fn row_to_detail(
    row: super::repo::AppointmentDetailRow,
    prescreen: PrescreenInfo,
) -> AppointmentDetail {
    AppointmentDetail {
        booking_id: row.booking_id,
        appointment_time: AppointmentTime {
            start_time: row.appointment_start.to_jiff().as_second(),
            end_time: row.appointment_end.to_jiff().as_second(),
        },
        status: row.appointment_status,
        booking_type: row.booking_type,
        consultation_channel: row.consultation_channel,
        patient: PartialUserIdentity {
            account_id: row.patient_account_id,
            profile_id: row.patient_profile_id,
        },
        doctor: PartialUserIdentity {
            account_id: row.doctor_account_id,
            profile_id: row.doctor_profile_id,
        },
        prescreen,
        payment_tx_id: row.payment_tx_id,
        payment_tx_ref_id: row.payment_tx_ref_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appointment::get_detail::repo::AppointmentDetailRow;
    use crate::repo::enums::{AppointmentStatusEnum, BookingTypeEnum, ConsultationChannelEnum};
    use crate::sys::crypto::kms::{Kms, KmsError, KmsResult};
    use base64::Engine;
    use std::sync::{Arc, Mutex};

    struct MockKms;

    #[async_trait::async_trait]
    impl Kms for MockKms {
        async fn encrypt(&self, plaintext: &[u8], _key_name: &str) -> KmsResult<Vec<u8>> {
            Ok(plaintext.to_vec())
        }

        async fn decrypt(&self, _ciphertext: &[u8], _key_name: &str) -> KmsResult<Vec<u8>> {
            unimplemented!(
                "MockKms decrypt is a test stub; if this panics, a test code path unexpectedly called decrypt — use GcpKmsService for tests that need real decryption"
            )
        }
    }

    struct FakeDecryptingKms {
        expected_key_name: String,
        expected_ciphertext: Vec<u8>,
        plaintext: Vec<u8>,
    }

    #[async_trait::async_trait]
    impl Kms for FakeDecryptingKms {
        async fn encrypt(&self, plaintext: &[u8], _key_name: &str) -> KmsResult<Vec<u8>> {
            Ok(plaintext.to_vec())
        }

        async fn decrypt(&self, ciphertext: &[u8], key_name: &str) -> KmsResult<Vec<u8>> {
            if key_name != self.expected_key_name {
                return Err(KmsError::DecryptionFailed(format!(
                    "unexpected key name: {key_name}"
                )));
            }
            if ciphertext != self.expected_ciphertext.as_slice() {
                return Err(KmsError::DecryptionFailed(
                    "unexpected ciphertext".to_string(),
                ));
            }

            Ok(self.plaintext.clone())
        }
    }

    /// Test-only repo that returns `Ok(None)` for any query.
    /// Used to construct `GetAppointmentDetailService` for the synchronous
    /// `decode_prescreen` tests below; will be reused by Slice 3 for
    /// `get_appointment_detail` AppointmentNotFound tests.
    struct StubRepo;

    #[async_trait::async_trait]
    impl crate::appointment::get_detail::repo::GetAppointmentDetailRepo for StubRepo {
        async fn get_appointment_detail(
            &self,
            _booking_id: &str,
        ) -> Result<Option<AppointmentDetailRow>, anyhow::Error> {
            Ok(None)
        }
    }

    struct SingleRowRepo {
        row: Mutex<Option<AppointmentDetailRow>>,
    }

    #[async_trait::async_trait]
    impl crate::appointment::get_detail::repo::GetAppointmentDetailRepo for SingleRowRepo {
        async fn get_appointment_detail(
            &self,
            _booking_id: &str,
        ) -> Result<Option<AppointmentDetailRow>, anyhow::Error> {
            Ok(self.row.lock().expect("test row mutex poisoned").take())
        }
    }

    struct CapturingRepo {
        booking_ids: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl crate::appointment::get_detail::repo::GetAppointmentDetailRepo for CapturingRepo {
        async fn get_appointment_detail(
            &self,
            booking_id: &str,
        ) -> Result<Option<AppointmentDetailRow>, anyhow::Error> {
            self.booking_ids
                .lock()
                .expect("test booking ids mutex poisoned")
                .push(booking_id.to_string());
            Ok(None)
        }
    }

    struct ErrorRepo;

    #[async_trait::async_trait]
    impl crate::appointment::get_detail::repo::GetAppointmentDetailRepo for ErrorRepo {
        async fn get_appointment_detail(
            &self,
            _booking_id: &str,
        ) -> Result<Option<AppointmentDetailRow>, anyhow::Error> {
            Err(anyhow::anyhow!("repo unavailable"))
        }
    }

    struct FailingKms;

    #[async_trait::async_trait]
    impl Kms for FailingKms {
        async fn encrypt(&self, plaintext: &[u8], _key_name: &str) -> KmsResult<Vec<u8>> {
            Ok(plaintext.to_vec())
        }

        async fn decrypt(&self, _ciphertext: &[u8], _key_name: &str) -> KmsResult<Vec<u8>> {
            Err(KmsError::DecryptionFailed("kms unavailable".to_string()))
        }
    }

    fn make_service() -> GetAppointmentDetailService {
        GetAppointmentDetailService::new(
            Arc::new(StubRepo),
            Arc::new(MockKms),
            "test-key".to_string(),
        )
    }

    fn ts(seconds: i64) -> jiff_sqlx::Timestamp {
        jiff_sqlx::Timestamp::from(
            jiff::Timestamp::from_second(seconds).expect("test timestamp should be valid"),
        )
    }

    fn prescreen_json() -> String {
        r#"{"symptom":"headache","duration":3,"durationUnit":"day","attachments":["a.jpg"],"allergies":["pollen"]}"#.to_string()
    }

    fn detail_row(prescreen_data_type: &str, prescreen_data: String) -> AppointmentDetailRow {
        AppointmentDetailRow {
            booking_id: "booking-123".to_string(),
            appointment_start: ts(1_701_000_000),
            appointment_end: ts(1_701_003_600),
            appointment_status: AppointmentStatusEnum::Booked,
            booking_type: BookingTypeEnum::Schedule,
            consultation_channel: ConsultationChannelEnum::Video,
            patient_account_id: 11,
            patient_profile_id: 12,
            doctor_account_id: 21,
            doctor_profile_id: 22,
            prescreen_data,
            prescreen_data_type: prescreen_data_type.to_string(),
            payment_tx_id: 31,
            payment_tx_ref_id: "payment-ref-123".to_string(),
        }
    }

    fn make_service_with_row(row: AppointmentDetailRow) -> GetAppointmentDetailService {
        GetAppointmentDetailService::new(
            Arc::new(SingleRowRepo {
                row: Mutex::new(Some(row)),
            }),
            Arc::new(MockKms),
            "test-key".to_string(),
        )
    }

    fn encrypted_prescreen_data(ciphertext: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(ciphertext)
    }

    #[tokio::test]
    async fn get_appointment_detail_returns_database_error_when_repo_fails() {
        let svc = GetAppointmentDetailService::new(
            Arc::new(ErrorRepo),
            Arc::new(MockKms),
            "test-key".to_string(),
        );
        let err = svc
            .get_appointment_detail("booking-123")
            .await
            .expect_err("repo failure should surface as DatabaseError");

        assert!(matches!(err, GetAppointmentDetailError::DatabaseError(_)));
    }

    #[tokio::test]
    async fn get_appointment_detail_returns_appointment_not_found_when_repo_returns_none() {
        let svc = make_service();
        let result = svc
            .get_appointment_detail("missing-booking")
            .await
            .expect("repo None should map to a successful AppointmentNotFound response");

        assert!(matches!(
            result,
            GetAppointmentDetailResponse::AppointmentNotFound
        ));
    }

    #[tokio::test]
    async fn get_appointment_detail_passes_booking_id_to_repo() {
        let repo = Arc::new(CapturingRepo {
            booking_ids: Mutex::new(vec![]),
        });
        let svc = GetAppointmentDetailService::new(
            repo.clone(),
            Arc::new(MockKms),
            "test-key".to_string(),
        );

        let result = svc
            .get_appointment_detail("booking-abc")
            .await
            .expect("capturing repo should return AppointmentNotFound");

        assert!(matches!(
            result,
            GetAppointmentDetailResponse::AppointmentNotFound
        ));
        assert_eq!(
            repo.booking_ids
                .lock()
                .expect("test booking ids mutex poisoned")
                .as_slice(),
            &["booking-abc".to_string()]
        );
    }

    #[tokio::test]
    async fn get_appointment_detail_maps_success_row_from_repo() {
        let svc = make_service_with_row(detail_row(PRESCREEN_TYPE_RAW_JSON, prescreen_json()));
        let result = svc
            .get_appointment_detail("booking-123")
            .await
            .expect("repo row should map to Success response");

        let GetAppointmentDetailResponse::Success(detail) = result else {
            panic!("expected Success response");
        };

        assert_eq!(detail.booking_id, "booking-123");
        assert_eq!(detail.appointment_time.start_time, 1_701_000_000);
        assert_eq!(detail.appointment_time.end_time, 1_701_003_600);
        assert!(matches!(detail.status, AppointmentStatusEnum::Booked));
        assert_eq!(detail.booking_type, BookingTypeEnum::Schedule);
        assert_eq!(detail.consultation_channel, ConsultationChannelEnum::Video);
        assert_eq!(detail.patient.account_id, 11);
        assert_eq!(detail.patient.profile_id, 12);
        assert_eq!(detail.doctor.account_id, 21);
        assert_eq!(detail.doctor.profile_id, 22);
        assert_eq!(detail.prescreen.symptom, "headache");
        assert_eq!(detail.prescreen.duration, 3);
        assert_eq!(detail.prescreen.duration_unit, "day");
        assert_eq!(detail.prescreen.attachments, vec!["a.jpg"]);
        assert_eq!(detail.prescreen.allergies, vec!["pollen"]);
        assert_eq!(detail.payment_tx_id, 31);
        assert_eq!(detail.payment_tx_ref_id, "payment-ref-123");
    }

    #[tokio::test]
    async fn decode_prescreen_raw_json_happy_path() {
        let svc = make_service();
        let result = svc
            .decode_prescreen(
                "RAW_JSON",
                r#"{"symptom":"headache","duration":3,"durationUnit":"day","attachments":["a.jpg"],"allergies":["pollen"]}"#,
            )
            .await
            .expect("should decode successfully");

        assert_eq!(result.symptom, "headache");
        assert_eq!(result.duration, 3);
        assert_eq!(result.duration_unit, "day");
        assert_eq!(result.attachments, vec!["a.jpg"]);
        assert_eq!(result.allergies, vec!["pollen"]);
    }

    #[tokio::test]
    async fn decode_prescreen_enc_gcp_kms_uses_fake_kms_plaintext() {
        let ciphertext = b"encrypted-prescreen".to_vec();
        let svc = GetAppointmentDetailService::new(
            Arc::new(StubRepo),
            Arc::new(FakeDecryptingKms {
                expected_key_name: "test-key".to_string(),
                expected_ciphertext: ciphertext.clone(),
                plaintext: prescreen_json().into_bytes(),
            }),
            "test-key".to_string(),
        );
        let encrypted_data = encrypted_prescreen_data(&ciphertext);

        let result = svc
            .decode_prescreen(PRESCREEN_TYPE_ENC_GCP_KMS, &encrypted_data)
            .await
            .expect("fake KMS plaintext should decode successfully");

        assert_eq!(result.symptom, "headache");
        assert_eq!(result.duration, 3);
        assert_eq!(result.duration_unit, "day");
        assert_eq!(result.attachments, vec!["a.jpg"]);
        assert_eq!(result.allergies, vec!["pollen"]);
    }

    #[tokio::test]
    async fn decode_prescreen_enc_gcp_kms_invalid_base64_errors() {
        let svc = make_service();
        let err = svc
            .decode_prescreen(PRESCREEN_TYPE_ENC_GCP_KMS, "not valid base64")
            .await
            .expect_err("invalid base64 should fail before KMS decrypt");

        assert!(matches!(
            err,
            GetAppointmentDetailError::PrescreenBase64Error(_)
        ));
    }

    #[tokio::test]
    async fn decode_prescreen_enc_gcp_kms_kms_failure_errors() {
        let ciphertext = b"encrypted-prescreen";
        let svc = GetAppointmentDetailService::new(
            Arc::new(StubRepo),
            Arc::new(FailingKms),
            "test-key".to_string(),
        );
        let err = svc
            .decode_prescreen(
                PRESCREEN_TYPE_ENC_GCP_KMS,
                &encrypted_prescreen_data(ciphertext),
            )
            .await
            .expect_err("KMS failure should map to PrescreenKmsError");

        assert!(matches!(
            err,
            GetAppointmentDetailError::PrescreenKmsError(_)
        ));
    }

    #[tokio::test]
    async fn decode_prescreen_enc_gcp_kms_invalid_utf8_errors() {
        let ciphertext = b"encrypted-prescreen".to_vec();
        let svc = GetAppointmentDetailService::new(
            Arc::new(StubRepo),
            Arc::new(FakeDecryptingKms {
                expected_key_name: "test-key".to_string(),
                expected_ciphertext: ciphertext.clone(),
                plaintext: vec![0xff],
            }),
            "test-key".to_string(),
        );
        let err = svc
            .decode_prescreen(
                PRESCREEN_TYPE_ENC_GCP_KMS,
                &encrypted_prescreen_data(&ciphertext),
            )
            .await
            .expect_err("invalid UTF-8 plaintext should fail");

        assert!(matches!(
            err,
            GetAppointmentDetailError::PrescreenUtf8Error(_)
        ));
    }

    #[tokio::test]
    async fn decode_prescreen_enc_gcp_kms_invalid_json_errors() {
        let ciphertext = b"encrypted-prescreen".to_vec();
        let svc = GetAppointmentDetailService::new(
            Arc::new(StubRepo),
            Arc::new(FakeDecryptingKms {
                expected_key_name: "test-key".to_string(),
                expected_ciphertext: ciphertext.clone(),
                plaintext: b"not-json".to_vec(),
            }),
            "test-key".to_string(),
        );
        let err = svc
            .decode_prescreen(
                PRESCREEN_TYPE_ENC_GCP_KMS,
                &encrypted_prescreen_data(&ciphertext),
            )
            .await
            .expect_err("invalid JSON plaintext should fail");

        assert!(matches!(
            err,
            GetAppointmentDetailError::PrescreenParseError(_)
        ));
    }

    #[tokio::test]
    async fn decode_prescreen_accepts_legacy_snake_case_duration_unit() {
        let svc = make_service();
        let result = svc
            .decode_prescreen(
                "RAW_JSON",
                r#"{"symptom":"headache","duration":3,"duration_unit":"day","attachments":["a.jpg"],"allergies":["pollen"]}"#,
            )
            .await
            .expect("should decode legacy prescreen shape");

        assert_eq!(result.duration_unit, "day");
    }

    #[tokio::test]
    async fn decode_prescreen_unknown_type_errors() {
        let svc = make_service();
        let result = svc.decode_prescreen("WEIRD_FORMAT", "{}").await;

        match result {
            Err(GetAppointmentDetailError::UnsupportedPrescreenDataType(s)) => {
                assert_eq!(s, "WEIRD_FORMAT");
            }
            other => panic!("expected UnsupportedPrescreenDataType, got {:?}", other),
        }
    }
}
