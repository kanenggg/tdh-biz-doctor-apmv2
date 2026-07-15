use std::sync::Arc;

use base64::Engine;

use super::model::{
    ConsultationSummary, ConsultationSummaryNote, ConsultationSummaryResponse, DoctorRef,
};
use super::repo::{ConsultationSummaryRepo, ConsultationSummaryRow};
use crate::appointment::types::AppointmentTime;
use crate::protocol::summary_note::SummarizationRequest;
use crate::repo::enums::AppointmentStatusEnum;
use crate::sys::crypto::kms::Kms;

/// `encrypted_data_type` written by the summarization service for KMS-encrypted notes.
pub(crate) const SUMMARY_NOTE_TYPE_ENC_GCP_KMS: &str = "DoctorSummaryNoteV1";

#[derive(Debug, thiserror::Error)]
pub enum ConsultationSummaryError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] anyhow::Error),
    #[error("Unsupported summary note data type: {0}")]
    UnsupportedDataType(String),
    #[error("Summary note base64 decode error: {0}")]
    Base64Error(#[from] base64::DecodeError),
    #[error("Summary note KMS error: {0}")]
    KmsError(String),
    #[error("Summary note UTF-8 decode error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
    #[error("Summary note JSON parse error: {0}")]
    ParseError(#[from] serde_json::Error),
}

#[derive(Clone)]
pub struct ConsultationSummaryService {
    repo: Arc<dyn ConsultationSummaryRepo>,
    kms: Arc<dyn Kms>,
    kms_key_name: String,
}

impl ConsultationSummaryService {
    pub fn new(
        repo: Arc<dyn ConsultationSummaryRepo>,
        kms: Arc<dyn Kms>,
        kms_key_name: String,
    ) -> Self {
        Self {
            repo,
            kms,
            kms_key_name,
        }
    }

    pub async fn get_consultation_summary(
        &self,
        booking_id: &str,
    ) -> Result<ConsultationSummaryResponse, ConsultationSummaryError> {
        let row = self
            .repo
            .get_consultation_summary(booking_id)
            .await
            .map_err(ConsultationSummaryError::DatabaseError)?;

        let Some(row) = row else {
            return Ok(ConsultationSummaryResponse::NotFound);
        };

        if !matches!(row.appointment_status, AppointmentStatusEnum::Fulfilled) {
            return Ok(ConsultationSummaryResponse::NotFulfilled);
        }

        // FULFILLED but no summary note is an invariant violation, not a normal "wrong state".
        let (Some(data), Some(data_type)) =
            (row.encrypted_data.clone(), row.encrypted_data_type.clone())
        else {
            tracing::warn!(
                booking_id = %booking_id,
                "FULFILLED appointment has no doctor_summary_note row"
            );
            return Ok(ConsultationSummaryResponse::NotFound);
        };

        let summary = self.decode_summary_note(&data_type, &data).await?;
        let detail = row_to_detail(row, summary);
        Ok(ConsultationSummaryResponse::Success(detail))
    }

    pub(crate) async fn decode_summary_note(
        &self,
        data_type: &str,
        data: &str,
    ) -> Result<SummarizationRequest, ConsultationSummaryError> {
        match data_type {
            SUMMARY_NOTE_TYPE_ENC_GCP_KMS => {
                let ciphertext = base64::engine::general_purpose::STANDARD.decode(data)?;
                let plaintext = self
                    .kms
                    .decrypt(&ciphertext, &self.kms_key_name)
                    .await
                    .map_err(|e| ConsultationSummaryError::KmsError(e.to_string()))?;
                let json = String::from_utf8(plaintext)?;
                let summary = serde_json::from_str(&json)?;
                Ok(summary)
            }
            other => Err(ConsultationSummaryError::UnsupportedDataType(
                other.to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::follow_up::FollowUp;
    use crate::protocol::summary_note::{DrugAllergy, DurationUnit, Icd10, SummarizationRequest};
    use crate::sys::crypto::kms::{Kms, KmsError, KmsResult};

    /// Identity KMS: `decrypt` returns the ciphertext unchanged, so a base64-encoded
    /// plaintext JSON round-trips through `decode_summary_note` without real crypto.
    struct IdentityKms;

    #[async_trait::async_trait]
    impl Kms for IdentityKms {
        async fn encrypt(&self, plaintext: &[u8], _key: &str) -> KmsResult<Vec<u8>> {
            Ok(plaintext.to_vec())
        }
        async fn decrypt(&self, ciphertext: &[u8], _key: &str) -> KmsResult<Vec<u8>> {
            Ok(ciphertext.to_vec())
        }
    }

    fn sample_summary() -> SummarizationRequest {
        SummarizationRequest {
            booking_id: "b-1".to_string(),
            prescription_id: Some(55),
            present_illness: "Rash".to_string(),
            chief_complaint: "Itching".to_string(),
            diagnosis: "Eczema".to_string(),
            recommendations: "Moisturize".to_string(),
            icd10: vec![Icd10 {
                code: "L30.9".to_string(),
                description: "Dermatitis, unspecified".to_string(),
            }],
            illness_duration: DurationUnit {
                unit: "Days".to_string(),
                value: 7,
            },
            note_to_staff: "Use non-fragrance soap".to_string(),
            follow_up: FollowUp::AsNeeded,
            drug_allergies: Some(vec![DrugAllergy {
                id: 9,
                display_name: "Penicillin".to_string(),
            }]),
        }
    }

    struct RowRepo(Option<ConsultationSummaryRow>);

    #[async_trait::async_trait]
    impl ConsultationSummaryRepo for RowRepo {
        async fn get_consultation_summary(
            &self,
            _booking_id: &str,
        ) -> Result<Option<ConsultationSummaryRow>, anyhow::Error> {
            Ok(self.0.clone())
        }
    }

    struct ErrorRepo;

    #[async_trait::async_trait]
    impl ConsultationSummaryRepo for ErrorRepo {
        async fn get_consultation_summary(
            &self,
            _booking_id: &str,
        ) -> Result<Option<ConsultationSummaryRow>, anyhow::Error> {
            Err(anyhow::anyhow!("repo unavailable"))
        }
    }

    struct FailingKms;

    #[async_trait::async_trait]
    impl Kms for FailingKms {
        async fn encrypt(&self, plaintext: &[u8], _key: &str) -> KmsResult<Vec<u8>> {
            Ok(plaintext.to_vec())
        }

        async fn decrypt(&self, _ciphertext: &[u8], _key: &str) -> KmsResult<Vec<u8>> {
            Err(KmsError::DecryptionFailed("kms unavailable".to_string()))
        }
    }

    fn ts(secs: i64) -> jiff_sqlx::Timestamp {
        jiff_sqlx::Timestamp::from(jiff::Timestamp::from_second(secs).unwrap())
    }

    fn make_row(status: AppointmentStatusEnum, with_note: bool) -> ConsultationSummaryRow {
        let (encrypted_data, encrypted_data_type, prescription_id) = if with_note {
            let json = serde_json::to_vec(&sample_summary()).unwrap();
            let b64 = base64::engine::general_purpose::STANDARD.encode(&json);
            (
                Some(b64),
                Some(SUMMARY_NOTE_TYPE_ENC_GCP_KMS.to_string()),
                Some(55),
            )
        } else {
            (None, None, None)
        };
        ConsultationSummaryRow {
            booking_id: "b-1".to_string(),
            appointment_start: ts(1_639_182_000),
            appointment_end: ts(1_639_188_000),
            appointment_status: status,
            consultation_channel: crate::repo::enums::ConsultationChannelEnum::Video,
            doctor_id: 7,
            doctor_account_id: 1001,
            doctor_profile_id: 2002,
            encrypted_data,
            encrypted_data_type,
            prescription_id,
        }
    }

    fn make_service(row: Option<ConsultationSummaryRow>) -> ConsultationSummaryService {
        ConsultationSummaryService::new(
            Arc::new(RowRepo(row)),
            Arc::new(IdentityKms),
            "k".to_string(),
        )
    }

    fn encoded_summary_note(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[tokio::test]
    async fn repo_error_returns_database_error() {
        let svc = ConsultationSummaryService::new(
            Arc::new(ErrorRepo),
            Arc::new(IdentityKms),
            "k".to_string(),
        );
        let err = svc
            .get_consultation_summary("b-1")
            .await
            .expect_err("repo failure should surface as DatabaseError");

        assert!(matches!(err, ConsultationSummaryError::DatabaseError(_)));
    }

    #[tokio::test]
    async fn missing_booking_returns_not_found() {
        let svc = make_service(None);
        let resp = svc.get_consultation_summary("b-1").await.unwrap();
        assert!(matches!(resp, ConsultationSummaryResponse::NotFound));
    }

    #[tokio::test]
    async fn non_fulfilled_returns_not_fulfilled() {
        let svc = make_service(Some(make_row(AppointmentStatusEnum::Booked, false)));
        let resp = svc.get_consultation_summary("b-1").await.unwrap();
        assert!(matches!(resp, ConsultationSummaryResponse::NotFulfilled));
    }

    #[tokio::test]
    async fn fulfilled_without_note_returns_not_found() {
        let svc = make_service(Some(make_row(AppointmentStatusEnum::Fulfilled, false)));
        let resp = svc.get_consultation_summary("b-1").await.unwrap();
        assert!(matches!(resp, ConsultationSummaryResponse::NotFound));
    }

    #[tokio::test]
    async fn fulfilled_with_note_returns_success() {
        let svc = make_service(Some(make_row(AppointmentStatusEnum::Fulfilled, true)));
        let resp = svc.get_consultation_summary("b-1").await.unwrap();
        match resp {
            ConsultationSummaryResponse::Success(d) => {
                assert_eq!(d.booking_id, "b-1");
                assert_eq!(d.appointment_time.start_time, 1_639_182_000);
                assert_eq!(d.appointment_time.end_time, 1_639_188_000);
                assert!(matches!(
                    d.consultation_channel,
                    crate::repo::enums::ConsultationChannelEnum::Video
                ));
                assert_eq!(d.doctor.doctor_id, 7);
                assert_eq!(d.doctor.doctor_account_id, 1001);
                assert_eq!(d.doctor.doctor_profile_id, 2002);
                assert_eq!(d.summary_note.prescription_id, Some(55));
                assert_eq!(d.summary_note.present_illness, "Rash");
                assert_eq!(d.summary_note.chief_complaint, "Itching");
                assert_eq!(d.summary_note.diagnosis, "Eczema");
                assert_eq!(d.summary_note.recommendations, "Moisturize");
                assert_eq!(d.summary_note.icd10.len(), 1);
                assert_eq!(d.summary_note.icd10[0].code, "L30.9");
                assert_eq!(
                    d.summary_note.icd10[0].description,
                    "Dermatitis, unspecified"
                );
                assert_eq!(d.summary_note.illness_duration.unit, "Days");
                assert_eq!(d.summary_note.illness_duration.value, 7);
                assert_eq!(d.summary_note.note_to_staff, "Use non-fragrance soap");
                let drug_allergies = d
                    .summary_note
                    .drug_allergies
                    .expect("drug allergies should map");
                assert_eq!(drug_allergies.len(), 1);
                assert_eq!(drug_allergies[0].id, 9);
                assert_eq!(drug_allergies[0].display_name, "Penicillin");
                assert!(matches!(d.follow_up, FollowUp::AsNeeded));
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn decode_summary_note_invalid_base64_errors() {
        let svc = make_service(None);
        let err = svc
            .decode_summary_note(SUMMARY_NOTE_TYPE_ENC_GCP_KMS, "not valid base64")
            .await
            .expect_err("invalid base64 should fail before KMS decrypt");

        assert!(matches!(err, ConsultationSummaryError::Base64Error(_)));
    }

    #[tokio::test]
    async fn decode_summary_note_kms_failure_errors() {
        let svc = ConsultationSummaryService::new(
            Arc::new(RowRepo(None)),
            Arc::new(FailingKms),
            "k".to_string(),
        );
        let err = svc
            .decode_summary_note(
                SUMMARY_NOTE_TYPE_ENC_GCP_KMS,
                &encoded_summary_note(b"encrypted-summary"),
            )
            .await
            .expect_err("KMS failure should map to KmsError");

        assert!(matches!(err, ConsultationSummaryError::KmsError(_)));
    }

    #[tokio::test]
    async fn decode_summary_note_invalid_utf8_errors() {
        let svc = make_service(None);
        let err = svc
            .decode_summary_note(
                SUMMARY_NOTE_TYPE_ENC_GCP_KMS,
                &encoded_summary_note(&[0xff]),
            )
            .await
            .expect_err("invalid UTF-8 plaintext should fail");

        assert!(matches!(err, ConsultationSummaryError::Utf8Error(_)));
    }

    #[tokio::test]
    async fn decode_summary_note_invalid_json_errors() {
        let svc = make_service(None);
        let err = svc
            .decode_summary_note(
                SUMMARY_NOTE_TYPE_ENC_GCP_KMS,
                &encoded_summary_note(b"not-json"),
            )
            .await
            .expect_err("invalid JSON plaintext should fail");

        assert!(matches!(err, ConsultationSummaryError::ParseError(_)));
    }

    #[tokio::test]
    async fn decode_unknown_type_errors() {
        let svc = make_service(None);
        let err = svc.decode_summary_note("WEIRD", "{}").await.unwrap_err();
        assert!(matches!(err, ConsultationSummaryError::UnsupportedDataType(t) if t == "WEIRD"));
    }
}

fn row_to_detail(
    row: ConsultationSummaryRow,
    summary: SummarizationRequest,
) -> ConsultationSummary {
    ConsultationSummary {
        booking_id: row.booking_id,
        appointment_time: AppointmentTime {
            start_time: row.appointment_start.to_jiff().as_second(),
            end_time: row.appointment_end.to_jiff().as_second(),
        },
        consultation_channel: row.consultation_channel,
        doctor: DoctorRef {
            doctor_id: row.doctor_id,
            doctor_account_id: row.doctor_account_id,
            doctor_profile_id: row.doctor_profile_id,
        },
        summary_note: ConsultationSummaryNote {
            prescription_id: row.prescription_id,
            present_illness: summary.present_illness,
            chief_complaint: summary.chief_complaint,
            diagnosis: summary.diagnosis,
            recommendations: summary.recommendations,
            icd10: summary.icd10,
            illness_duration: summary.illness_duration,
            note_to_staff: summary.note_to_staff,
            drug_allergies: summary.drug_allergies,
        },
        follow_up: summary.follow_up,
    }
}
