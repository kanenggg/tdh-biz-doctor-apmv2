use std::sync::Arc;

use common_rs::twilio::TwilioStatusCallback;

use crate::common::tdh_protocol::common::PartialUserIdentity;
use crate::common::tdh_protocol::consultation::{ConsultationEvent, SessionMessage};
use crate::consultation::common::SessionDetails;
use crate::infra::event::EventPublisher;
use crate::provider_callback::repo::{
    CallbackParticipantRole, ProviderCallbackRepo, TwilioCallbackSessionContext,
};

#[derive(Debug, thiserror::Error)]
pub enum ProviderCallbackError {
    #[error("unsupported callback")]
    Unsupported,
    #[error("missing appointment id")]
    MissingAppointmentId,
    #[error("missing participant identity")]
    MissingParticipantIdentity,
    #[error("unsupported participant identity: {0}")]
    UnsupportedParticipantIdentity(String),
    #[error("session not found")]
    SessionNotFound,
    #[error("callback room sid mismatch")]
    RoomSidMismatch,
    #[error("callback participant identity mismatch")]
    ParticipantIdentityMismatch,
    #[error("repository error: {0}")]
    Repository(#[from] anyhow::Error),
}

#[derive(Clone)]
pub struct ProviderCallbackService {
    repo: Arc<dyn ProviderCallbackRepo>,
    event_publisher: Arc<dyn EventPublisher>,
}

impl ProviderCallbackService {
    pub fn new(
        repo: Arc<dyn ProviderCallbackRepo>,
        event_publisher: Arc<dyn EventPublisher>,
    ) -> Self {
        Self {
            repo,
            event_publisher,
        }
    }

    pub async fn handle_twilio_callback(
        &self,
        payload: TwilioStatusCallback,
    ) -> Result<(), ProviderCallbackError> {
        if !payload.is_participant_disconnected() {
            return Err(ProviderCallbackError::Unsupported);
        }

        let appointment_id = appointment_id_from_room_name(payload.room_name.as_deref())
            .ok_or(ProviderCallbackError::MissingAppointmentId)?;
        let participant_identity = payload
            .participant_identity
            .as_deref()
            .ok_or(ProviderCallbackError::MissingParticipantIdentity)?;
        let role = role_from_identity(participant_identity)?;
        let event_type = match role {
            CallbackParticipantRole::Patient => "PatientDisconnected",
            CallbackParticipantRole::Doctor => "DoctorDisconnected",
        };
        let session_context = self
            .repo
            .get_twilio_callback_context(&appointment_id)
            .await?
            .ok_or(ProviderCallbackError::SessionNotFound)?;
        validate_callback_matches_session(&payload, participant_identity, role, &session_context)?;

        let provider_event_id = payload.provider_event_id();
        let now = jiff::Timestamp::now().as_second();
        let raw_payload = serde_json::to_value(&payload)
            .map_err(|e| ProviderCallbackError::Repository(anyhow::anyhow!(e)))?;

        let is_new_callback = self
            .repo
            .insert_callback_event(
                &provider_event_id,
                Some(&appointment_id),
                event_type,
                Some(participant_identity),
                raw_payload,
            )
            .await?;
        if !is_new_callback {
            return Ok(());
        }

        let was_first_disconnect = self
            .repo
            .mark_participant_disconnected(&appointment_id, role, now)
            .await?;
        if !was_first_disconnect {
            return Ok(());
        }

        self.publish_disconnected(role, &session_context.details, now)
            .await
    }

    async fn publish_disconnected(
        &self,
        role: CallbackParticipantRole,
        session_details: &SessionDetails,
        disconnected_at: i64,
    ) -> Result<(), ProviderCallbackError> {
        let patient_identity = PartialUserIdentity {
            account_id: session_details.patient_account_id,
            user_profile_id: session_details.patient_profile_id,
            tenant_id: session_details.tenant_id,
            oidc_user_id: None,
        };
        let message = match role {
            CallbackParticipantRole::Patient => SessionMessage::PatientDisconnected {
                booking_id: session_details.booking_id.clone(),
                patient_identity,
                doctor_id: session_details.doctor_id,
                disconnected_at,
            },
            CallbackParticipantRole::Doctor => SessionMessage::DoctorDisconnected {
                booking_id: session_details.booking_id.clone(),
                patient_identity,
                doctor_id: session_details.doctor_id,
                disconnected_at,
            },
        };

        self.event_publisher
            .publish_consultation_event(ConsultationEvent::SessionMessage(message))
            .await
            .map_err(ProviderCallbackError::Repository)
    }
}

fn validate_callback_matches_session(
    payload: &TwilioStatusCallback,
    participant_identity: &str,
    role: CallbackParticipantRole,
    session_context: &TwilioCallbackSessionContext,
) -> Result<(), ProviderCallbackError> {
    if let Some(expected_room_sid) = session_context
        .room_sid
        .as_deref()
        .filter(|sid| !sid.is_empty())
    {
        if payload.room_sid.as_deref() != Some(expected_room_sid) {
            return Err(ProviderCallbackError::RoomSidMismatch);
        }
    }

    let expected_identity = match role {
        CallbackParticipantRole::Patient => format!(
            "patient_{}_{}",
            session_context.details.patient_account_id, session_context.details.patient_profile_id
        ),
        CallbackParticipantRole::Doctor => format!(
            "doctor_{}_{}",
            session_context.details.doctor_id, session_context.details.doctor_profile_id
        ),
    };

    if participant_identity != expected_identity {
        return Err(ProviderCallbackError::ParticipantIdentityMismatch);
    }

    Ok(())
}

fn appointment_id_from_room_name(room_name: Option<&str>) -> Option<String> {
    room_name
        .and_then(|name| name.strip_prefix("mordee_twilio_video_"))
        .map(ToString::to_string)
}

fn role_from_identity(identity: &str) -> Result<CallbackParticipantRole, ProviderCallbackError> {
    if identity.starts_with("patient_") {
        Ok(CallbackParticipantRole::Patient)
    } else if identity.starts_with("doctor_") {
        Ok(CallbackParticipantRole::Doctor)
    } else {
        Err(ProviderCallbackError::UnsupportedParticipantIdentity(
            identity.to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use crate::common::tdh_protocol::common::meeting_provider::MeetingProvider;
    use crate::doctor_timeslot::configuration_events::model::DoctorTimeslotConfigChangedEvent;

    struct FakeProviderCallbackRepo {
        insert_callback_result: bool,
        mark_disconnect_result: bool,
        session_details: Mutex<Option<SessionDetails>>,
        insert_calls: AtomicUsize,
        mark_disconnect_calls: AtomicUsize,
        get_session_details_calls: AtomicUsize,
        marked_roles: Mutex<Vec<CallbackParticipantRole>>,
    }

    impl FakeProviderCallbackRepo {
        fn new(
            insert_callback_result: bool,
            mark_disconnect_result: bool,
            session_details: Option<SessionDetails>,
        ) -> Self {
            Self {
                insert_callback_result,
                mark_disconnect_result,
                session_details: Mutex::new(session_details),
                insert_calls: AtomicUsize::new(0),
                mark_disconnect_calls: AtomicUsize::new(0),
                get_session_details_calls: AtomicUsize::new(0),
                marked_roles: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl ProviderCallbackRepo for FakeProviderCallbackRepo {
        async fn insert_callback_event(
            &self,
            _provider_event_id: &str,
            _appointment_id: Option<&str>,
            _event_type: &str,
            _participant_identity: Option<&str>,
            _payload: serde_json::Value,
        ) -> Result<bool, anyhow::Error> {
            self.insert_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.insert_callback_result)
        }

        async fn mark_participant_disconnected(
            &self,
            _appointment_id: &str,
            role: CallbackParticipantRole,
            _disconnected_at: i64,
        ) -> Result<bool, anyhow::Error> {
            self.mark_disconnect_calls.fetch_add(1, Ordering::SeqCst);
            self.marked_roles
                .lock()
                .expect("marked roles mutex should not be poisoned")
                .push(role);
            Ok(self.mark_disconnect_result)
        }

        async fn get_session_details(
            &self,
            _appointment_id: &str,
        ) -> Result<Option<SessionDetails>, anyhow::Error> {
            self.get_session_details_calls
                .fetch_add(1, Ordering::SeqCst);
            Ok(self
                .session_details
                .lock()
                .expect("session details mutex should not be poisoned")
                .take())
        }

        async fn get_twilio_callback_context(
            &self,
            _appointment_id: &str,
        ) -> Result<Option<TwilioCallbackSessionContext>, anyhow::Error> {
            self.get_session_details_calls
                .fetch_add(1, Ordering::SeqCst);
            Ok(self
                .session_details
                .lock()
                .expect("session details mutex should not be poisoned")
                .take()
                .map(|details| TwilioCallbackSessionContext {
                    details,
                    room_sid: Some("RM123".to_string()),
                }))
        }
    }

    #[derive(Default)]
    struct FakeEventPublisher {
        consultation_events: Mutex<Vec<ConsultationEvent>>,
    }

    impl FakeEventPublisher {
        fn consultation_events(&self) -> Vec<ConsultationEvent> {
            self.consultation_events
                .lock()
                .expect("consultation events mutex should not be poisoned")
                .clone()
        }
    }

    #[async_trait::async_trait]
    impl EventPublisher for FakeEventPublisher {
        async fn publish_consultation_event(
            &self,
            event: ConsultationEvent,
        ) -> Result<(), anyhow::Error> {
            self.consultation_events
                .lock()
                .expect("consultation events mutex should not be poisoned")
                .push(event);
            Ok(())
        }

        async fn publish_doctor_timeslot_config_changed_event(
            &self,
            _event: DoctorTimeslotConfigChangedEvent,
        ) -> Result<(), anyhow::Error> {
            unreachable!("doctor timeslot config events should not be published")
        }
    }

    fn session_details() -> SessionDetails {
        SessionDetails {
            appointment_id: "appointment-1".to_string(),
            booking_id: "booking-1".to_string(),
            patient_account_id: 10,
            patient_profile_id: 20,
            tenant_id: 30,
            doctor_id: 40,
            doctor_profile_id: 50,
            session_provider: MeetingProvider::Twilio,
            session_chat_id: Some("CH123".to_string()),
        }
    }

    fn twilio_disconnect_callback(participant_identity: &str) -> TwilioStatusCallback {
        TwilioStatusCallback {
            status_callback_event: Some("participant-disconnected".to_string()),
            room_name: Some("mordee_twilio_video_appointment-1".to_string()),
            room_sid: Some("RM123".to_string()),
            participant_identity: Some(participant_identity.to_string()),
            participant_status: None,
            timestamp: None,
            sequence_number: Some("1".to_string()),
        }
    }

    fn twilio_unsupported_callback() -> TwilioStatusCallback {
        TwilioStatusCallback {
            status_callback_event: Some("room-ended".to_string()),
            room_name: Some("mordee_twilio_video_appointment-1".to_string()),
            room_sid: Some("RM123".to_string()),
            participant_identity: Some("patient_10_20".to_string()),
            participant_status: None,
            timestamp: None,
            sequence_number: Some("1".to_string()),
        }
    }

    #[test]
    fn extracts_appointment_id_from_twilio_room_name() {
        assert_eq!(
            appointment_id_from_room_name(Some("mordee_twilio_video_BK1")).as_deref(),
            Some("BK1")
        );
    }

    #[test]
    fn parses_role_from_token_identity() {
        assert_eq!(
            role_from_identity("patient_1_2").unwrap(),
            CallbackParticipantRole::Patient
        );
        assert_eq!(
            role_from_identity("doctor_1_2").unwrap(),
            CallbackParticipantRole::Doctor
        );
    }

    #[tokio::test]
    async fn unsupported_callback_returns_error_without_repo_or_publish_side_effects() {
        let repo = Arc::new(FakeProviderCallbackRepo::new(
            true,
            true,
            Some(session_details()),
        ));
        let event_publisher = Arc::new(FakeEventPublisher::default());
        let service = ProviderCallbackService::new(repo.clone(), event_publisher.clone());

        let result = service
            .handle_twilio_callback(twilio_unsupported_callback())
            .await;

        assert!(matches!(result, Err(ProviderCallbackError::Unsupported)));
        assert_eq!(repo.insert_calls.load(Ordering::SeqCst), 0);
        assert_eq!(repo.mark_disconnect_calls.load(Ordering::SeqCst), 0);
        assert_eq!(repo.get_session_details_calls.load(Ordering::SeqCst), 0);
        assert!(event_publisher.consultation_events().is_empty());
    }

    #[tokio::test]
    async fn missing_appointment_id_returns_error_without_repo_or_publish_side_effects() {
        let repo = Arc::new(FakeProviderCallbackRepo::new(
            true,
            true,
            Some(session_details()),
        ));
        let event_publisher = Arc::new(FakeEventPublisher::default());
        let service = ProviderCallbackService::new(repo.clone(), event_publisher.clone());
        let mut callback = twilio_disconnect_callback("patient_10_20");
        callback.room_name = Some("unexpected-room-name".to_string());

        let result = service.handle_twilio_callback(callback).await;

        assert!(matches!(
            result,
            Err(ProviderCallbackError::MissingAppointmentId)
        ));
        assert_eq!(repo.insert_calls.load(Ordering::SeqCst), 0);
        assert_eq!(repo.mark_disconnect_calls.load(Ordering::SeqCst), 0);
        assert_eq!(repo.get_session_details_calls.load(Ordering::SeqCst), 0);
        assert!(event_publisher.consultation_events().is_empty());
    }

    #[tokio::test]
    async fn missing_participant_identity_returns_error_without_repo_or_publish_side_effects() {
        let repo = Arc::new(FakeProviderCallbackRepo::new(
            true,
            true,
            Some(session_details()),
        ));
        let event_publisher = Arc::new(FakeEventPublisher::default());
        let service = ProviderCallbackService::new(repo.clone(), event_publisher.clone());
        let mut callback = twilio_disconnect_callback("patient_10_20");
        callback.participant_identity = None;

        let result = service.handle_twilio_callback(callback).await;

        assert!(matches!(
            result,
            Err(ProviderCallbackError::MissingParticipantIdentity)
        ));
        assert_eq!(repo.insert_calls.load(Ordering::SeqCst), 0);
        assert_eq!(repo.mark_disconnect_calls.load(Ordering::SeqCst), 0);
        assert_eq!(repo.get_session_details_calls.load(Ordering::SeqCst), 0);
        assert!(event_publisher.consultation_events().is_empty());
    }

    #[tokio::test]
    async fn unsupported_participant_identity_returns_error_without_repo_or_publish_side_effects() {
        let repo = Arc::new(FakeProviderCallbackRepo::new(
            true,
            true,
            Some(session_details()),
        ));
        let event_publisher = Arc::new(FakeEventPublisher::default());
        let service = ProviderCallbackService::new(repo.clone(), event_publisher.clone());

        let result = service
            .handle_twilio_callback(twilio_disconnect_callback("guest_10_20"))
            .await;

        assert!(matches!(
            result,
            Err(ProviderCallbackError::UnsupportedParticipantIdentity(identity))
                if identity == "guest_10_20"
        ));
        assert_eq!(repo.insert_calls.load(Ordering::SeqCst), 0);
        assert_eq!(repo.mark_disconnect_calls.load(Ordering::SeqCst), 0);
        assert_eq!(repo.get_session_details_calls.load(Ordering::SeqCst), 0);
        assert!(event_publisher.consultation_events().is_empty());
    }

    #[tokio::test]
    async fn duplicate_callback_returns_ok_without_disconnect_or_publish_side_effects() {
        let repo = Arc::new(FakeProviderCallbackRepo::new(
            false,
            true,
            Some(session_details()),
        ));
        let event_publisher = Arc::new(FakeEventPublisher::default());
        let service = ProviderCallbackService::new(repo.clone(), event_publisher.clone());

        let result = service
            .handle_twilio_callback(twilio_disconnect_callback("patient_10_20"))
            .await;

        assert!(result.is_ok());
        assert_eq!(repo.insert_calls.load(Ordering::SeqCst), 1);
        assert_eq!(repo.mark_disconnect_calls.load(Ordering::SeqCst), 0);
        assert_eq!(repo.get_session_details_calls.load(Ordering::SeqCst), 1);
        assert!(event_publisher.consultation_events().is_empty());
    }

    #[tokio::test]
    async fn already_marked_disconnect_returns_ok_without_publishing_event() {
        let repo = Arc::new(FakeProviderCallbackRepo::new(
            true,
            false,
            Some(session_details()),
        ));
        let event_publisher = Arc::new(FakeEventPublisher::default());
        let service = ProviderCallbackService::new(repo.clone(), event_publisher.clone());

        let result = service
            .handle_twilio_callback(twilio_disconnect_callback("patient_10_20"))
            .await;

        assert!(result.is_ok());
        assert_eq!(repo.insert_calls.load(Ordering::SeqCst), 1);
        assert_eq!(repo.mark_disconnect_calls.load(Ordering::SeqCst), 1);
        assert_eq!(repo.get_session_details_calls.load(Ordering::SeqCst), 1);
        assert!(event_publisher.consultation_events().is_empty());
    }

    #[tokio::test]
    async fn mismatched_participant_identity_returns_error_without_mutation_or_publish() {
        let repo = Arc::new(FakeProviderCallbackRepo::new(
            true,
            true,
            Some(session_details()),
        ));
        let event_publisher = Arc::new(FakeEventPublisher::default());
        let service = ProviderCallbackService::new(repo.clone(), event_publisher.clone());

        let result = service
            .handle_twilio_callback(twilio_disconnect_callback("patient_999_20"))
            .await;

        assert!(matches!(
            result,
            Err(ProviderCallbackError::ParticipantIdentityMismatch)
        ));
        assert_eq!(repo.insert_calls.load(Ordering::SeqCst), 0);
        assert_eq!(repo.mark_disconnect_calls.load(Ordering::SeqCst), 0);
        assert_eq!(repo.get_session_details_calls.load(Ordering::SeqCst), 1);
        assert!(event_publisher.consultation_events().is_empty());
    }

    #[tokio::test]
    async fn mismatched_room_sid_returns_error_without_mutation_or_publish() {
        let repo = Arc::new(FakeProviderCallbackRepo::new(
            true,
            true,
            Some(session_details()),
        ));
        let event_publisher = Arc::new(FakeEventPublisher::default());
        let service = ProviderCallbackService::new(repo.clone(), event_publisher.clone());
        let mut callback = twilio_disconnect_callback("patient_10_20");
        callback.room_sid = Some("RM999".to_string());

        let result = service.handle_twilio_callback(callback).await;

        assert!(matches!(
            result,
            Err(ProviderCallbackError::RoomSidMismatch)
        ));
        assert_eq!(repo.insert_calls.load(Ordering::SeqCst), 0);
        assert_eq!(repo.mark_disconnect_calls.load(Ordering::SeqCst), 0);
        assert_eq!(repo.get_session_details_calls.load(Ordering::SeqCst), 1);
        assert!(event_publisher.consultation_events().is_empty());
    }

    #[tokio::test]
    async fn first_patient_disconnect_publishes_patient_disconnected_event() {
        let repo = Arc::new(FakeProviderCallbackRepo::new(
            true,
            true,
            Some(session_details()),
        ));
        let event_publisher = Arc::new(FakeEventPublisher::default());
        let service = ProviderCallbackService::new(repo.clone(), event_publisher.clone());

        let result = service
            .handle_twilio_callback(twilio_disconnect_callback("patient_10_20"))
            .await;

        assert!(result.is_ok());
        assert_eq!(repo.mark_disconnect_calls.load(Ordering::SeqCst), 1);
        assert_eq!(repo.get_session_details_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            repo.marked_roles
                .lock()
                .expect("marked roles mutex should not be poisoned")
                .as_slice(),
            &[CallbackParticipantRole::Patient]
        );

        let events = event_publisher.consultation_events();
        assert_eq!(events.len(), 1);
        let ConsultationEvent::SessionMessage(SessionMessage::PatientDisconnected {
            booking_id,
            patient_identity,
            doctor_id,
            disconnected_at,
        }) = &events[0]
        else {
            panic!("expected PatientDisconnected event, got {:?}", events[0]);
        };
        assert_eq!(booking_id, "booking-1");
        assert_eq!(*doctor_id, 40);
        assert_eq!(patient_identity.account_id, 10);
        assert_eq!(patient_identity.user_profile_id, 20);
        assert_eq!(patient_identity.tenant_id, 30);
        assert_eq!(patient_identity.oidc_user_id, None);
        assert!(*disconnected_at > 0);
    }

    #[tokio::test]
    async fn first_doctor_disconnect_publishes_doctor_disconnected_event() {
        let repo = Arc::new(FakeProviderCallbackRepo::new(
            true,
            true,
            Some(session_details()),
        ));
        let event_publisher = Arc::new(FakeEventPublisher::default());
        let service = ProviderCallbackService::new(repo.clone(), event_publisher.clone());

        let result = service
            .handle_twilio_callback(twilio_disconnect_callback("doctor_40_50"))
            .await;

        assert!(result.is_ok());
        assert_eq!(repo.mark_disconnect_calls.load(Ordering::SeqCst), 1);
        assert_eq!(repo.get_session_details_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            repo.marked_roles
                .lock()
                .expect("marked roles mutex should not be poisoned")
                .as_slice(),
            &[CallbackParticipantRole::Doctor]
        );

        let events = event_publisher.consultation_events();
        assert_eq!(events.len(), 1);
        let ConsultationEvent::SessionMessage(SessionMessage::DoctorDisconnected {
            booking_id,
            patient_identity,
            doctor_id,
            disconnected_at,
        }) = &events[0]
        else {
            panic!("expected DoctorDisconnected event, got {:?}", events[0]);
        };
        assert_eq!(booking_id, "booking-1");
        assert_eq!(*doctor_id, 40);
        assert_eq!(patient_identity.account_id, 10);
        assert_eq!(patient_identity.user_profile_id, 20);
        assert_eq!(patient_identity.tenant_id, 30);
        assert_eq!(patient_identity.oidc_user_id, None);
        assert!(*disconnected_at > 0);
    }
}
