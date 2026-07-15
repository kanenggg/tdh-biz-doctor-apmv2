use std::sync::Arc;

use jiff::{Timestamp, civil::Date, tz::TimeZone};

use crate::doctor_timeslot::configuration_events::model::{
    DoctorTimeslotConfigChangeType, DoctorTimeslotConfigChangedEvent,
};

use super::model::{
    AvailabilityResponse, DoctorAuthContext, DoctorConfigIdentity, ScheduleAvailableConfig,
    SuccessResponse, UpdateScheduleConfigResponse,
};
use super::repo::{ConsultationConfigRepo, ConsultationConfigRepoError};
use super::validate::{ScheduleConfigValidationError, validate_schedule_config};
use super::window::{drop_past_specific_dates, retain_forward_window};

#[derive(Debug, thiserror::Error)]
pub enum ConsultationConfigError {
    #[error("{0}")]
    BadRequest(String),
    #[error("doctor identity is not provisioned for this account/profile")]
    DoctorIdentityNotProvisioned,
    #[error("repository error: {0}")]
    Repository(#[from] ConsultationConfigRepoError),
    #[error("event publish error: {0}")]
    EventPublish(#[from] anyhow::Error),
}

#[derive(Clone)]
pub struct ConsultationConfigService {
    repo: Arc<dyn ConsultationConfigRepo>,
}

impl ConsultationConfigService {
    pub fn new(repo: Arc<dyn ConsultationConfigRepo>) -> Self {
        Self { repo }
    }

    pub async fn get_schedule_config(
        &self,
        context: DoctorAuthContext,
    ) -> Result<ScheduleAvailableConfig, ConsultationConfigError> {
        let identity = self.resolve_identity(context).await?;
        let mut config = self
            .repo
            .get_schedule_config(identity)
            .await?
            .unwrap_or_default();

        retain_forward_window(&mut config, bangkok_today());

        Ok(config)
    }

    pub async fn update_schedule_config(
        &self,
        context: DoctorAuthContext,
        mut req: ScheduleAvailableConfig,
    ) -> Result<UpdateScheduleConfigResponse, ConsultationConfigError> {
        let identity = self.resolve_identity(context).await?;

        match validate_schedule_config(&req) {
            Ok(()) => {}
            Err(ScheduleConfigValidationError::Invalid(message)) => {
                return Err(ConsultationConfigError::BadRequest(message));
            }
            Err(ScheduleConfigValidationError::ConflictTimeOverlap { days }) => {
                return Ok(UpdateScheduleConfigResponse::ConflictTimeOverlap { days });
            }
        }

        drop_past_specific_dates(&mut req, bangkok_today());

        let event = DoctorTimeslotConfigChangedEvent::new(
            identity,
            DoctorTimeslotConfigChangeType::TimeslotConfiguration,
            None,
        );
        self.repo
            .save_schedule_config_and_enqueue(identity, &req, &event)
            .await?;

        Ok(UpdateScheduleConfigResponse::Success)
    }

    pub async fn set_schedule_availability(
        &self,
        context: DoctorAuthContext,
        available: bool,
    ) -> Result<SuccessResponse, ConsultationConfigError> {
        let identity = self.resolve_identity(context).await?;
        let event = DoctorTimeslotConfigChangedEvent::new(
            identity,
            DoctorTimeslotConfigChangeType::ScheduleAvailability,
            Some(available),
        );
        self.repo
            .set_schedule_availability_and_enqueue(identity, available, &event)
            .await?;

        Ok(SuccessResponse::Success)
    }

    pub async fn set_instant_availability(
        &self,
        context: DoctorAuthContext,
        available: bool,
    ) -> Result<SuccessResponse, ConsultationConfigError> {
        let identity = self.resolve_identity(context).await?;
        let event = DoctorTimeslotConfigChangedEvent::new(
            identity,
            DoctorTimeslotConfigChangeType::InstantAvailability,
            Some(available),
        );
        self.repo
            .set_instant_availability_and_enqueue(identity, available, &event)
            .await?;

        Ok(SuccessResponse::Success)
    }

    pub async fn get_availability(
        &self,
        context: DoctorAuthContext,
    ) -> Result<AvailabilityResponse, ConsultationConfigError> {
        let identity = self.resolve_identity(context).await?;
        let availability = self.repo.get_availability(identity).await?;

        Ok(AvailabilityResponse::success(
            availability.schedule_available,
            availability.instant_available,
        ))
    }

    async fn resolve_identity(
        &self,
        context: DoctorAuthContext,
    ) -> Result<DoctorConfigIdentity, ConsultationConfigError> {
        self.repo
            .resolve_current_doctor_identity(context.doctor_account_id, context.doctor_profile_id)
            .await?
            .ok_or(ConsultationConfigError::DoctorIdentityNotProvisioned)
    }
}

fn bangkok_today() -> Date {
    let tz = TimeZone::get("Asia/Bangkok").expect("Asia/Bangkok is a valid IANA timezone");
    Timestamp::now().to_zoned(tz).date()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::common::tdh_protocol::consultation::ConsultationEvent;
    use crate::consultation_config::model::{ConsultationAvailability, TimePeriod};
    use crate::doctor_timeslot::configuration_events::model::DoctorTimeslotConfigEventType;
    use crate::infra::event::{EventPublisher, NoOpEventPublisher};

    struct RecordingRepo {
        identity: Option<DoctorConfigIdentity>,
        identity_lookups: Mutex<Vec<(i64, i64)>>,
        availability_lookups: Mutex<Vec<DoctorConfigIdentity>>,
        schedule_config_saves: Mutex<Vec<(DoctorConfigIdentity, ScheduleAvailableConfig)>>,
        schedule_availability_updates: Mutex<Vec<(DoctorConfigIdentity, bool)>>,
        instant_availability_updates: Mutex<Vec<(DoctorConfigIdentity, bool)>>,
        outbox_events: Mutex<Vec<DoctorTimeslotConfigChangedEvent>>,
        fail_schedule_config_save: bool,
    }

    impl RecordingRepo {
        fn new(identity: Option<DoctorConfigIdentity>) -> Self {
            Self {
                identity,
                identity_lookups: Mutex::new(Vec::new()),
                availability_lookups: Mutex::new(Vec::new()),
                schedule_config_saves: Mutex::new(Vec::new()),
                schedule_availability_updates: Mutex::new(Vec::new()),
                instant_availability_updates: Mutex::new(Vec::new()),
                outbox_events: Mutex::new(Vec::new()),
                fail_schedule_config_save: false,
            }
        }

        fn failing_schedule_config_save(identity: Option<DoctorConfigIdentity>) -> Self {
            Self {
                fail_schedule_config_save: true,
                ..Self::new(identity)
            }
        }

        fn identity_lookups(&self) -> Vec<(i64, i64)> {
            self.identity_lookups
                .lock()
                .expect("mutex poisoned")
                .clone()
        }

        fn availability_lookups(&self) -> Vec<DoctorConfigIdentity> {
            self.availability_lookups
                .lock()
                .expect("mutex poisoned")
                .clone()
        }

        fn schedule_config_saves(&self) -> Vec<(DoctorConfigIdentity, ScheduleAvailableConfig)> {
            self.schedule_config_saves
                .lock()
                .expect("mutex poisoned")
                .clone()
        }

        fn schedule_availability_updates(&self) -> Vec<(DoctorConfigIdentity, bool)> {
            self.schedule_availability_updates
                .lock()
                .expect("mutex poisoned")
                .clone()
        }

        fn instant_availability_updates(&self) -> Vec<(DoctorConfigIdentity, bool)> {
            self.instant_availability_updates
                .lock()
                .expect("mutex poisoned")
                .clone()
        }

        fn outbox_events(&self) -> Vec<DoctorTimeslotConfigChangedEvent> {
            self.outbox_events.lock().expect("mutex poisoned").clone()
        }
    }

    #[async_trait::async_trait]
    impl ConsultationConfigRepo for RecordingRepo {
        async fn resolve_current_doctor_identity(
            &self,
            doctor_account_id: i64,
            doctor_profile_id: i64,
        ) -> Result<Option<DoctorConfigIdentity>, ConsultationConfigRepoError> {
            self.identity_lookups
                .lock()
                .expect("mutex poisoned")
                .push((doctor_account_id, doctor_profile_id));

            Ok(self.identity)
        }

        async fn get_schedule_config(
            &self,
            _identity: DoctorConfigIdentity,
        ) -> Result<Option<ScheduleAvailableConfig>, ConsultationConfigRepoError> {
            unreachable!("schedule config lookup is outside this test")
        }

        async fn save_schedule_config(
            &self,
            identity: DoctorConfigIdentity,
            config: &ScheduleAvailableConfig,
        ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
            if self.fail_schedule_config_save {
                return Err(ConsultationConfigRepoError::Database(
                    sqlx::Error::RowNotFound,
                ));
            }

            self.schedule_config_saves
                .lock()
                .expect("mutex poisoned")
                .push((identity, config.clone()));

            Ok(ConsultationAvailability::default())
        }

        async fn set_schedule_availability(
            &self,
            identity: DoctorConfigIdentity,
            available: bool,
        ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
            self.schedule_availability_updates
                .lock()
                .expect("mutex poisoned")
                .push((identity, available));

            Ok(ConsultationAvailability::default())
        }

        async fn set_instant_availability(
            &self,
            identity: DoctorConfigIdentity,
            available: bool,
        ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
            self.instant_availability_updates
                .lock()
                .expect("mutex poisoned")
                .push((identity, available));

            Ok(ConsultationAvailability::default())
        }

        async fn save_schedule_config_and_enqueue(
            &self,
            identity: DoctorConfigIdentity,
            config: &ScheduleAvailableConfig,
            event: &DoctorTimeslotConfigChangedEvent,
        ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
            let availability = self.save_schedule_config(identity, config).await?;
            self.outbox_events
                .lock()
                .expect("mutex poisoned")
                .push(event.clone());
            Ok(availability)
        }

        async fn set_schedule_availability_and_enqueue(
            &self,
            identity: DoctorConfigIdentity,
            available: bool,
            event: &DoctorTimeslotConfigChangedEvent,
        ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
            let availability = self.set_schedule_availability(identity, available).await?;
            self.outbox_events
                .lock()
                .expect("mutex poisoned")
                .push(event.clone());
            Ok(availability)
        }

        async fn set_instant_availability_and_enqueue(
            &self,
            identity: DoctorConfigIdentity,
            available: bool,
            event: &DoctorTimeslotConfigChangedEvent,
        ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
            let availability = self.set_instant_availability(identity, available).await?;
            self.outbox_events
                .lock()
                .expect("mutex poisoned")
                .push(event.clone());
            Ok(availability)
        }

        async fn get_availability(
            &self,
            identity: DoctorConfigIdentity,
        ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
            self.availability_lookups
                .lock()
                .expect("mutex poisoned")
                .push(identity);

            Ok(ConsultationAvailability {
                schedule_available: true,
                instant_available: false,
            })
        }
    }

    #[derive(Default)]
    struct RecordingEventPublisher {
        doctor_timeslot_events: Mutex<Vec<DoctorTimeslotConfigChangedEvent>>,
        fail_doctor_timeslot_publish: bool,
    }

    impl RecordingEventPublisher {
        fn failing_doctor_timeslot_publish() -> Self {
            Self {
                doctor_timeslot_events: Mutex::new(Vec::new()),
                fail_doctor_timeslot_publish: true,
            }
        }

        fn doctor_timeslot_events(&self) -> Vec<DoctorTimeslotConfigChangedEvent> {
            self.doctor_timeslot_events
                .lock()
                .expect("mutex poisoned")
                .clone()
        }
    }

    #[async_trait::async_trait]
    impl EventPublisher for RecordingEventPublisher {
        async fn publish_consultation_event(
            &self,
            _event: ConsultationEvent,
        ) -> Result<(), anyhow::Error> {
            Ok(())
        }

        async fn publish_doctor_timeslot_config_changed_event(
            &self,
            event: DoctorTimeslotConfigChangedEvent,
        ) -> Result<(), anyhow::Error> {
            if self.fail_doctor_timeslot_publish {
                anyhow::bail!("doctor timeslot event publish failed");
            }

            self.doctor_timeslot_events
                .lock()
                .expect("mutex poisoned")
                .push(event);
            Ok(())
        }
    }

    fn service_with_repo(repo: Arc<RecordingRepo>) -> ConsultationConfigService {
        ConsultationConfigService::new(repo)
    }

    fn service_with_publisher(
        repo: Arc<RecordingRepo>,
        event_publisher: Arc<RecordingEventPublisher>,
    ) -> ConsultationConfigService {
        let _ = event_publisher;
        ConsultationConfigService::new(repo)
    }

    fn doctor_identity() -> DoctorConfigIdentity {
        DoctorConfigIdentity {
            doctor_id: uuid::uuid!("018f1414-5e0e-7c2a-b908-7b1967f2b401"),
            doctor_account_id: 42,
            doctor_profile_id: 84,
        }
    }

    fn auth_context() -> DoctorAuthContext {
        DoctorAuthContext {
            doctor_account_id: 42,
            doctor_profile_id: 84,
        }
    }

    fn assert_doctor_timeslot_event(
        event: &DoctorTimeslotConfigChangedEvent,
        identity: DoctorConfigIdentity,
        change_type: DoctorTimeslotConfigChangeType,
        is_active: Option<bool>,
    ) {
        assert!(!event.event_id.is_empty());
        assert!(!event.changed_at.is_empty());
        assert_eq!(
            event.event_type,
            DoctorTimeslotConfigEventType::DoctorTimeslotConfigurationChanged
        );
        assert_eq!(event.doctor, identity.event_doctor());
        assert_eq!(event.source.service, "tdh-biz-doctor-apmv2/consultation-rs");
        assert_eq!(event.source.version, "v1");
        assert_eq!(event.change_type, change_type);
        assert_eq!(event.is_active, is_active);
    }

    #[tokio::test]
    async fn consultation_config_update_schedule_config_publishes_timeslot_configuration_event() {
        let identity = doctor_identity();
        let repo = Arc::new(RecordingRepo::new(Some(identity)));
        let service = service_with_repo(repo.clone());
        let config = ScheduleAvailableConfig::default();

        let response = service
            .update_schedule_config(auth_context(), config.clone())
            .await
            .expect("schedule config update should succeed");

        assert_eq!(response, UpdateScheduleConfigResponse::Success);
        assert_eq!(repo.identity_lookups(), vec![(42, 84)]);
        assert_eq!(repo.schedule_config_saves(), vec![(identity, config)]);
        let events = repo.outbox_events();
        assert_eq!(events.len(), 1);
        assert_doctor_timeslot_event(
            &events[0],
            identity,
            DoctorTimeslotConfigChangeType::TimeslotConfiguration,
            None,
        );
    }

    #[tokio::test]
    async fn consultation_config_set_schedule_availability_publishes_schedule_event() {
        let identity = doctor_identity();
        let repo = Arc::new(RecordingRepo::new(Some(identity)));
        let service = service_with_repo(repo.clone());

        let response = service
            .set_schedule_availability(auth_context(), true)
            .await
            .expect("schedule availability update should succeed");

        assert_eq!(response, SuccessResponse::Success);
        assert_eq!(repo.identity_lookups(), vec![(42, 84)]);
        assert_eq!(repo.schedule_availability_updates(), vec![(identity, true)]);
        let events = repo.outbox_events();
        assert_eq!(events.len(), 1);
        assert_doctor_timeslot_event(
            &events[0],
            identity,
            DoctorTimeslotConfigChangeType::ScheduleAvailability,
            Some(true),
        );
    }

    #[tokio::test]
    async fn consultation_config_set_instant_availability_publishes_instant_event() {
        let identity = doctor_identity();
        let repo = Arc::new(RecordingRepo::new(Some(identity)));
        let service = service_with_repo(repo.clone());

        let response = service
            .set_instant_availability(auth_context(), false)
            .await
            .expect("instant availability update should succeed");

        assert_eq!(response, SuccessResponse::Success);
        assert_eq!(repo.identity_lookups(), vec![(42, 84)]);
        assert_eq!(repo.instant_availability_updates(), vec![(identity, false)]);
        let events = repo.outbox_events();
        assert_eq!(events.len(), 1);
        assert_doctor_timeslot_event(
            &events[0],
            identity,
            DoctorTimeslotConfigChangeType::InstantAvailability,
            Some(false),
        );
    }

    #[tokio::test]
    async fn consultation_config_update_schedule_config_overlap_does_not_publish_event() {
        let identity = doctor_identity();
        let repo = Arc::new(RecordingRepo::new(Some(identity)));
        let service = service_with_repo(repo.clone());
        let mut config = ScheduleAvailableConfig::default();
        config.days_of_week.insert(
            1,
            vec![
                TimePeriod {
                    start_time: 60,
                    end_time: 120,
                },
                TimePeriod {
                    start_time: 90,
                    end_time: 150,
                },
            ],
        );

        let response = service
            .update_schedule_config(auth_context(), config)
            .await
            .expect("overlap should return a domain response, not an error");

        assert_eq!(
            response,
            UpdateScheduleConfigResponse::ConflictTimeOverlap { days: vec![1] }
        );
        assert_eq!(repo.identity_lookups(), vec![(42, 84)]);
        assert!(repo.schedule_config_saves().is_empty());
        assert!(repo.outbox_events().is_empty());
    }

    #[tokio::test]
    async fn consultation_config_repo_failure_does_not_publish_event() {
        let identity = doctor_identity();
        let repo = Arc::new(RecordingRepo::failing_schedule_config_save(Some(identity)));
        let service = service_with_repo(repo.clone());

        let error = service
            .update_schedule_config(auth_context(), ScheduleAvailableConfig::default())
            .await
            .expect_err("repository failure should be returned before event publication");

        assert!(matches!(error, ConsultationConfigError::Repository(_)));
        assert_eq!(repo.identity_lookups(), vec![(42, 84)]);
        assert!(repo.schedule_config_saves().is_empty());
        assert!(repo.outbox_events().is_empty());
    }

    #[tokio::test]
    async fn consultation_config_enqueue_is_independent_of_direct_publishing() {
        let identity = doctor_identity();
        let repo = Arc::new(RecordingRepo::new(Some(identity)));
        let service = service_with_repo(repo.clone());

        let response = service
            .set_schedule_availability(auth_context(), true)
            .await
            .expect("the durable outbox enqueue should not depend on direct publication");

        assert_eq!(response, SuccessResponse::Success);
        assert_eq!(repo.identity_lookups(), vec![(42, 84)]);
        assert_eq!(repo.schedule_availability_updates(), vec![(identity, true)]);
        assert_eq!(repo.outbox_events().len(), 1);
    }

    #[tokio::test]
    async fn consultation_config_resolves_doctor_identity_from_auth_context_for_availability() {
        let identity = DoctorConfigIdentity {
            doctor_id: uuid::uuid!("018f1414-5e0e-7c2a-b908-7b1967f2b401"),
            doctor_account_id: 42,
            doctor_profile_id: 84,
        };
        let repo = Arc::new(RecordingRepo::new(Some(identity)));
        let service = service_with_repo(repo.clone());

        let response = service
            .get_availability(DoctorAuthContext {
                doctor_account_id: 42,
                doctor_profile_id: 84,
            })
            .await
            .expect("availability lookup should succeed");

        assert_eq!(repo.identity_lookups(), vec![(42, 84)]);
        assert_eq!(repo.availability_lookups(), vec![identity]);
        assert_eq!(response, AvailabilityResponse::success(true, false));
    }

    #[tokio::test]
    async fn consultation_config_returns_not_provisioned_when_doctor_identity_missing() {
        let repo = Arc::new(RecordingRepo::new(None));
        let service = service_with_repo(repo.clone());

        let error = service
            .get_availability(DoctorAuthContext {
                doctor_account_id: 42,
                doctor_profile_id: 84,
            })
            .await
            .expect_err("missing identity should fail before availability lookup");

        assert!(matches!(
            error,
            ConsultationConfigError::DoctorIdentityNotProvisioned
        ));
        assert_eq!(repo.identity_lookups(), vec![(42, 84)]);
        assert!(repo.availability_lookups().is_empty());
    }
}
