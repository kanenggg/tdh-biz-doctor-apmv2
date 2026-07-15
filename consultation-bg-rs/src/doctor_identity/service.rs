use std::sync::Arc;

use super::{
    model::{DoctorProfileEvent, DoctorProfileEventValidationError},
    repo::{
        DoctorDeactivationProjection, DoctorIdentityRepo, DoctorProfileProjection,
        ProjectionContractConflict,
    },
};

#[derive(Debug, thiserror::Error)]
pub enum DoctorIdentityError {
    #[error("invalid doctor profile event: {0}")]
    InvalidEvent(#[from] DoctorProfileEventValidationError),
    #[error("doctor profile event contract conflict")]
    ContractConflict,
    #[error("approved doctor profile event has no committed consultation configuration")]
    MissingApprovedConfig,
    #[error("database error: {0}")]
    Database(#[from] anyhow::Error),
}

pub struct DoctorIdentityService {
    repo: Arc<dyn DoctorIdentityRepo>,
    allowed_schema_versions: Vec<i32>,
}

impl DoctorIdentityService {
    pub fn new(repo: Arc<dyn DoctorIdentityRepo>, allowed_schema_versions: Vec<i32>) -> Self {
        Self {
            repo,
            allowed_schema_versions,
        }
    }

    pub async fn handle_event(&self, event: DoctorProfileEvent) -> Result<(), DoctorIdentityError> {
        event.validate(&self.allowed_schema_versions)?;
        let approved_config = match &event {
            DoctorProfileEvent::DoctorProfileApproved { .. } => Some(event_config(&event)?),
            DoctorProfileEvent::DoctorProfileDeactivated { .. } => None,
        };
        match event {
            DoctorProfileEvent::DoctorProfileApproved {
                event_id,
                doctor_id,
                doctor_account_id,
                doctor_profile_id,
                is_active,
                profile_version,
                occurred_at,
                ..
            } => {
                self.repo
                    .apply_projection(DoctorProfileProjection {
                        event_id,
                        doctor_id,
                        doctor_account_id: i64::from(doctor_account_id),
                        doctor_profile_id: i64::from(doctor_profile_id),
                        is_active,
                        profile_version,
                        source_occurred_at: occurred_at,
                        consultation_config: approved_config
                            .ok_or(DoctorIdentityError::MissingApprovedConfig)?,
                    })
                    .await
                    .map_err(map_repo_error)?;
                tracing::info!(%doctor_id, doctor_account_id, doctor_profile_id, "doctor identity approved/upserted");
            }
            DoctorProfileEvent::DoctorProfileDeactivated {
                event_id,
                doctor_id,
                doctor_account_id,
                doctor_profile_id,
                profile_version,
                occurred_at,
                ..
            } => {
                self.repo
                    .deactivate(DoctorDeactivationProjection {
                        event_id,
                        doctor_id,
                        doctor_account_id: i64::from(doctor_account_id),
                        doctor_profile_id: i64::from(doctor_profile_id),
                        profile_version,
                        source_occurred_at: occurred_at,
                    })
                    .await
                    .map_err(map_repo_error)?;
            }
        }
        Ok(())
    }
}

fn event_config(
    event: &DoctorProfileEvent,
) -> Result<super::model::DoctorServiceConfig, DoctorIdentityError> {
    event.committed_service_config().map_err(Into::into)
}

fn map_repo_error(error: anyhow::Error) -> DoctorIdentityError {
    if error.downcast_ref::<ProjectionContractConflict>().is_some() {
        DoctorIdentityError::ContractConflict
    } else {
        DoctorIdentityError::Database(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Debug, PartialEq)]
    struct RecordedCall {
        event_id: String,
        version: Option<i64>,
        occurred_at: i64,
        fee_amount: String,
    }

    #[derive(Default)]
    struct RecordingDoctorIdentityRepo {
        calls: Mutex<Vec<RecordedCall>>,
    }

    #[async_trait::async_trait]
    impl DoctorIdentityRepo for RecordingDoctorIdentityRepo {
        async fn apply_projection(
            &self,
            projection: DoctorProfileProjection,
        ) -> Result<(), anyhow::Error> {
            self.calls.lock().unwrap().push(RecordedCall {
                event_id: projection.event_id,
                version: projection.profile_version,
                occurred_at: projection.source_occurred_at,
                fee_amount: projection.consultation_config.fee_amount.to_string(),
            });
            Ok(())
        }

        async fn deactivate(
            &self,
            _projection: DoctorDeactivationProjection,
        ) -> Result<(), anyhow::Error> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn approved_fixture_maps_top_level_config_and_unversioned_ordering_key() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let service = DoctorIdentityService::new(repo.clone(), vec![2]);
        let event = serde_json::from_str(include_str!(
            "../../../fixtures/doctor_profile_approved_origin_main.json"
        ))
        .unwrap();

        service.handle_event(event).await.unwrap();

        assert_eq!(
            *repo.calls.lock().unwrap(),
            vec![RecordedCall {
                event_id: "evt-1".to_string(),
                version: None,
                occurred_at: 1_718_668_800,
                fee_amount: "650.00".to_string(),
            }]
        );
    }
}
