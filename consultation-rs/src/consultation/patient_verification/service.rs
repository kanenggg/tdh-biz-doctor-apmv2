use std::sync::Arc;

use crate::{
    consultation::patient_verification::repo::PatientVerificationRepoPsql,
    infra::event::EventPublisher,
};

#[derive(Debug, thiserror::Error)]
pub enum PatientVerificationError {
    #[error("consultation not found or unauthorized")]
    NotFoundOrUnauthorized,
    #[error("repository error: {0}")]
    Repository(#[from] anyhow::Error),
}

#[derive(Clone)]
pub struct PatientVerificationService {
    repo: Arc<PatientVerificationRepoPsql>,
    event_publisher: Arc<dyn EventPublisher>,
}

impl PatientVerificationService {
    pub fn new(
        repo: Arc<PatientVerificationRepoPsql>,
        event_publisher: Arc<dyn EventPublisher>,
    ) -> Self {
        Self {
            repo,
            event_publisher,
        }
    }

    pub async fn match_handle(
        &self,
        booking_id: &str,
        doctor_profile_id: i64,
    ) -> Result<u64, PatientVerificationError> {
        let result = self
            .repo
            .add_patient_verification(booking_id, doctor_profile_id)
            .await
            .map_err(PatientVerificationError::Repository)?;

        if result == 0 {
            return Err(PatientVerificationError::NotFoundOrUnauthorized);
        }

        Ok(result)
    }

    pub async fn miss_match_handle(
        &self,
        booking_id: &str,
        doctor_profile_id: i64,
    ) -> Result<u64, PatientVerificationError> {
        let result = self
            .repo
            .add_patient_verification(booking_id, doctor_profile_id)
            .await
            .map_err(PatientVerificationError::Repository)?;

        if result == 0 {
            return Err(PatientVerificationError::NotFoundOrUnauthorized);
        }

        Ok(result)
    }
}
