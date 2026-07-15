use std::sync::Arc;

use common_rs::tdh_protocol::consultation::event::ConsultationEvent;

use crate::common::tdh_protocol::{
    common::{PartialUserIdentity, meeting_provider::MeetingProvider},
    consultation::{SessionMessage, SessionParticipant, TerminationCode},
};

use crate::{
    common::TwilioClient, consultation::end_session::repo::EndSessionRepo,
    infra::event::EventPublisher,
};

#[derive(Clone)]
pub struct EndSessionService {
    event_publisher: Arc<dyn EventPublisher>,
    twilio_client: Arc<dyn TwilioClient>,
    repo: Arc<dyn EndSessionRepo>,
}

impl EndSessionService {
    pub fn new(
        event_publisher: Arc<dyn EventPublisher>,
        twlio_client: Arc<dyn TwilioClient>,
        repo: Arc<dyn EndSessionRepo>,
    ) -> Self {
        Self {
            event_publisher,
            twilio_client: twlio_client,
            repo,
        }
    }

    pub async fn doctor_end_session(
        &self,
        appointment_id: &str,
        doctor_profile_id: i64,
        termination_code: TerminationCode,
    ) -> Result<u64, anyhow::Error> {
        let session_details = self
            .repo
            .get_session_details(appointment_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("Session not found for appointment_id: {}", appointment_id)
            })?;

        let rows_affected = self
            .repo
            .complete_consultation_status_to_done(appointment_id, doctor_profile_id)
            .await?;

        if rows_affected == 0 {
            tracing::info!(
                "Session already ended or appointment not found. appointment_id: {}",
                appointment_id
            );
            return Ok(0);
        }

        let terminated_at = jiff::Timestamp::now().as_second();

        match session_details.session_provider {
            MeetingProvider::Twilio => {
                if let Some(chat_sid) = session_details.session_chat_id {
                    if let Err(e) = self.twilio_client.close_conversation(chat_sid).await {
                        tracing::warn!("Failed to close Twilio conversation: {}", e);
                    }
                }
            }
            MeetingProvider::TokBox => {
                tracing::warn!(
                    "TokBox provider not supported for ending session. appointment_id: {}",
                    appointment_id
                );
            }
        }

        let patient_identity = PartialUserIdentity {
            account_id: session_details.patient_account_id,
            user_profile_id: session_details.patient_profile_id,
            tenant_id: session_details.tenant_id,
            oidc_user_id: None,
        };

        let message = SessionMessage::SessionTerminated {
            booking_id: session_details.booking_id.clone(),
            patient_identity,
            doctor_id: session_details.doctor_id,
            termination_code,
            terminated_by: SessionParticipant::Doctor,
            terminated_at,
        };

        self.event_publisher
            .publish_consultation_event(ConsultationEvent::SessionMessage(message))
            .await
            .inspect_err(|e| {
                tracing::error!("Failed to publish session terminated event: {}", e)
            })?;

        Ok(rows_affected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use common_rs::twilio::{CreateConversationResponse, CreateRoomResponse};

    use crate::{
        consultation::common::SessionDetails,
        doctor_timeslot::configuration_events::model::DoctorTimeslotConfigChangedEvent,
    };

    struct FakeEndSessionRepo {
        session_details: Mutex<Option<SessionDetails>>,
        rows_affected: u64,
        complete_calls: AtomicUsize,
    }

    impl FakeEndSessionRepo {
        fn new(session_details: SessionDetails, rows_affected: u64) -> Self {
            Self {
                session_details: Mutex::new(Some(session_details)),
                rows_affected,
                complete_calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl EndSessionRepo for FakeEndSessionRepo {
        async fn complete_consultation_status_to_done(
            &self,
            _appointment_id: &str,
            _doctor_profile_id: i64,
        ) -> Result<u64, anyhow::Error> {
            self.complete_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.rows_affected)
        }

        async fn get_session_details(
            &self,
            _appointment_id: &str,
        ) -> Result<Option<SessionDetails>, anyhow::Error> {
            Ok(self
                .session_details
                .lock()
                .expect("session details mutex should not be poisoned")
                .take())
        }
    }

    #[derive(Default)]
    struct FakeTwilioClient {
        close_calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl TwilioClient for FakeTwilioClient {
        async fn create_video_room(
            &self,
            _name: String,
            _record: bool,
        ) -> Result<CreateRoomResponse, crate::common::TwilioError> {
            unreachable!("create_video_room should not be called by end_session")
        }

        async fn fetch_room(
            &self,
            _name: String,
        ) -> Result<CreateRoomResponse, crate::common::TwilioError> {
            unreachable!("fetch_room should not be called by end_session")
        }

        async fn complete_room(&self, _room_sid: String) -> Result<(), crate::common::TwilioError> {
            unreachable!("complete_room should not be called by end_session")
        }

        async fn create_voice_room(
            &self,
            _name: String,
            _record: bool,
        ) -> Result<CreateRoomResponse, crate::common::TwilioError> {
            unreachable!("create_voice_room should not be called by end_session")
        }

        async fn create_conversation(
            &self,
            _name: String,
        ) -> Result<CreateConversationResponse, crate::common::TwilioError> {
            unreachable!("create_conversation should not be called by end_session")
        }

        async fn join_conversation(
            &self,
            _conversation_sid: String,
            _identity: String,
        ) -> Result<(), crate::common::TwilioError> {
            unreachable!("join_conversation should not be called by end_session")
        }

        async fn close_conversation(
            &self,
            _conversation_sid: String,
        ) -> Result<(), crate::common::TwilioError> {
            self.close_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn create_access_token(
            &self,
            _room_name: String,
            _chat_service_sid: Option<String>,
            _identity: String,
            _expires_at: Option<i64>,
        ) -> Result<String, common_rs::twilio::JwtError> {
            unreachable!("create_access_token should not be called by end_session")
        }
    }

    #[derive(Default)]
    struct FakeEventPublisher {
        consultation_event_calls: AtomicUsize,
        consultation_events: Mutex<Vec<ConsultationEvent>>,
    }

    #[async_trait::async_trait]
    impl EventPublisher for FakeEventPublisher {
        async fn publish_consultation_event(
            &self,
            event: ConsultationEvent,
        ) -> Result<(), anyhow::Error> {
            self.consultation_event_calls.fetch_add(1, Ordering::SeqCst);
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
            unreachable!("doctor timeslot config event should not be published by end_session")
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

    #[tokio::test]
    async fn doctor_end_session_returns_zero_without_side_effects_when_repo_updates_zero_rows() {
        let event_publisher = Arc::new(FakeEventPublisher::default());
        let twilio_client = Arc::new(FakeTwilioClient::default());
        let repo = Arc::new(FakeEndSessionRepo::new(session_details(), 0));
        let service =
            EndSessionService::new(event_publisher.clone(), twilio_client.clone(), repo.clone());

        let result = service
            .doctor_end_session("appointment-1", 50, TerminationCode::BothPartiesAbsent)
            .await;

        assert_eq!(result.expect("end session should succeed"), 0);
        assert_eq!(repo.complete_calls.load(Ordering::SeqCst), 1);
        assert_eq!(twilio_client.close_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            event_publisher
                .consultation_event_calls
                .load(Ordering::SeqCst),
            0
        );
    }

    #[tokio::test]
    async fn doctor_end_session_emits_requested_patient_verification_mismatch_termination_code() {
        let event_publisher = Arc::new(FakeEventPublisher::default());
        let twilio_client = Arc::new(FakeTwilioClient::default());
        let repo = Arc::new(FakeEndSessionRepo::new(session_details(), 1));
        let service =
            EndSessionService::new(event_publisher.clone(), twilio_client.clone(), repo.clone());

        let result = service
            .doctor_end_session(
                "appointment-1",
                50,
                TerminationCode::PatientVerificationMismatch,
            )
            .await;

        assert_eq!(result.expect("end session should succeed"), 1);
        assert_eq!(repo.complete_calls.load(Ordering::SeqCst), 1);
        assert_eq!(twilio_client.close_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            event_publisher
                .consultation_event_calls
                .load(Ordering::SeqCst),
            1
        );

        let events = event_publisher
            .consultation_events
            .lock()
            .expect("consultation events mutex should not be poisoned");
        assert_eq!(events.len(), 1);

        let ConsultationEvent::SessionMessage(SessionMessage::SessionTerminated {
            booking_id,
            termination_code,
            terminated_by,
            ..
        }) = &events[0]
        else {
            panic!("expected SessionTerminated event, got {:?}", events[0]);
        };

        assert_eq!(booking_id, "booking-1");
        assert!(matches!(
            termination_code,
            TerminationCode::PatientVerificationMismatch
        ));
        assert!(matches!(terminated_by, SessionParticipant::Doctor));
    }
}
