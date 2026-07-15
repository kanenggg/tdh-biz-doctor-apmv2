use crate::common::tdh_protocol::appointment::v2::payment_transaction::PaymentChannel;
use crate::common::tdh_protocol::common::PartialUserIdentity;
use crate::common::tdh_protocol::consultation::v2::session_info::{
    GetSessionInfoResult, ProviderSessionInfo, SessionReady, TwilioSessionInfo,
};
use crate::common::tdh_protocol::consultation::{
    ConsultationEvent, SessionMessage, SessionParticipant,
};
use crate::common::tdh_protocol::iam::user_identity::{AccountType, UserIdentity};
use common_rs::twilio::CreateRoomResponse;
use std::sync::Arc;

use crate::common::TwilioClient;
use crate::consultation::common::{DbConsultationSession, SessionDetails};
use crate::consultation::session_info::repo::{
    ParticipantJoinRecord, SessionManagementRepo, SessionParticipantRole,
};
use crate::infra::event::EventPublisher;
use crate::repo::enums::{AppointmentStatusEnum, ConsultationChannelEnum, DbMeetingProvider};
use crate::repo::provider_session_info::{self, SessionData};
use crate::sys::config::AppConfig;
use thiserror::Error;

#[derive(Error, Debug)]
pub struct ResultWarp(pub GetSessionInfoResult);

// Implement Display manually so we don't have to touch the original enum
impl std::fmt::Display for ResultWarp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            GetSessionInfoResult::SessionReady(_) => write!(f, "Session is ready"),
            GetSessionInfoResult::SessionNotFound => write!(f, "Session not found"),
            GetSessionInfoResult::SessionIsFinished => write!(f, "Session is finished"),
            GetSessionInfoResult::SessionIsNotReady => write!(f, "Session is not ready"),
            GetSessionInfoResult::Unauthorized => write!(f, "Unauthorized access"),
            GetSessionInfoResult::ProviderIsOutOfService(e) => {
                write!(f, "Provider is out of service: {}", e)
            }
        }
    }
}

#[derive(Error, Debug)]
pub enum SessionError {
    #[error(transparent)]
    GetSessionInfoResult(#[from] ResultWarp),
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Database error: {0}")]
    DatabaseError(#[from] anyhow::Error),
    #[error("Invalid session status in database")]
    InvalidSessionStatus,
    #[error("Invalid appointment status: {0}")]
    InvalidAppointmentStatus(String),
    #[error("JWT generation error: {0}")]
    JwtError(String),
    #[error("Twilio error: {0}")]
    TwilioError(String),
}

impl From<GetSessionInfoResult> for SessionError {
    fn from(value: GetSessionInfoResult) -> Self {
        Self::GetSessionInfoResult(ResultWarp(value))
    }
}

impl From<crate::common::TwilioError> for SessionError {
    fn from(e: crate::common::TwilioError) -> Self {
        SessionError::TwilioError(e.to_string())
    }
}

#[derive(Clone)]
pub struct GetOrCreateConsultSessionService {
    repo: Arc<dyn SessionManagementRepo>,
    twilio_client: Arc<dyn TwilioClient>,
    event_publisher: Arc<dyn EventPublisher>,
    config: AppConfig,
}

struct CreateTwilioSessionWrap {
    room: Option<CreateRoomResponse>,
    conversation: Option<(String, String)>,
}

impl From<CreateTwilioSessionWrap> for SessionData {
    fn from(value: CreateTwilioSessionWrap) -> Self {
        let (chat_sid, chat_service_sid) =
            value.conversation.unwrap_or((String::new(), String::new()));
        SessionData::Twilio(provider_session_info::TwilioSessionInfo {
            recording_url: value
                .room
                .as_ref()
                .map(|r| format!("https://video.twilio.com/v1/Rooms/{}/Recordings", r.sid))
                .unwrap_or_default(),
            session_chat_id: chat_sid,
            chat_recording_url: String::new(),
            session_chat_service_id: chat_service_sid,
            session_room_id: value.room.as_ref().map(|room| room.sid.clone()),
            session_room_name: value.room.as_ref().map(|room| room.unique_name.clone()),
        })
    }
}

impl GetOrCreateConsultSessionService {
    pub fn new(
        repo: Arc<dyn SessionManagementRepo>,
        twilio_client: Arc<dyn TwilioClient>,
        event_publisher: Arc<dyn EventPublisher>,
        config: AppConfig,
    ) -> Self {
        Self {
            repo,
            twilio_client,
            event_publisher,
            config,
        }
    }
    pub async fn get_or_create_session(
        &self,
        user_id: UserIdentity,
        appointment_id: &str,
    ) -> Result<GetSessionInfoResult, SessionError> {
        let consultation_session = self
            .repo
            .get_appointment_session(&user_id, appointment_id)
            .await?
            .ok_or(GetSessionInfoResult::SessionNotFound)
            .and_then(|session| {
                if self.is_authorized(&user_id, &session) {
                    Ok(session)
                } else {
                    Err(GetSessionInfoResult::Unauthorized)
                }
            })
            .and_then(|session| {
                if self.is_within_acceptable_time(
                    session.consultation_start_time,
                    session.consultation_end_time,
                    jiff::Timestamp::now().as_second(),
                ) {
                    Ok(session)
                } else {
                    tracing::info!("Session is not acceptable time");
                    Err(GetSessionInfoResult::SessionIsNotReady)
                }
            })
            .and_then(|session| match session.appointment_status {
                AppointmentStatusEnum::Booked | AppointmentStatusEnum::Arrived => Ok(session),
                AppointmentStatusEnum::Fulfilled | AppointmentStatusEnum::Cancelled => {
                    Err(GetSessionInfoResult::SessionIsFinished)
                }
                _ => Err(GetSessionInfoResult::SessionIsNotReady),
            })?;

        let canonical_appointment_id = consultation_session.appointment_id.clone();
        let (provider_sesion_info, session_created): (ProviderSessionInfo, bool) =
            match &consultation_session.session_provider_name {
                DbMeetingProvider::Twilio => {
                    self.handle_twilio_session(
                        &user_id,
                        &canonical_appointment_id,
                        &consultation_session,
                    )
                    .await
                }
                DbMeetingProvider::TokBox => Err(SessionError::InvalidSessionStatus),
            }?;

        self.publish_session_lifecycle_events(
            &user_id,
            &canonical_appointment_id,
            &consultation_session,
            session_created,
        )
        .await?;

        let is_patient_verification_required: Option<bool> = consultation_session
            .payment_channels
            .as_ref()
            .map(|channels| {
                channels
                    .0
                    .iter()
                    .any(|c| matches!(c, PaymentChannel::Insurance { .. }))
            });

        Ok(GetSessionInfoResult::SessionReady(SessionReady {
            session_info: provider_sesion_info,
            session_start_time: consultation_session.consultation_start_time,
            session_end_time: consultation_session.consultation_end_time,
            is_facial_verified: consultation_session.is_facial_verified,
            is_required_patient_verification: is_patient_verification_required,
            session_channel: consultation_session.consultation_channel.into(),
            rtdb_access: None,
        }))
    }

    async fn handle_twilio_session(
        &self,
        user_id: &UserIdentity,
        appointment_id: &str,
        consultation_session: &DbConsultationSession,
    ) -> Result<(ProviderSessionInfo, bool), SessionError> {
        let role_name = if user_id.account_type == AccountType::Doctor {
            "doctor"
        } else {
            "patient"
        };
        let room_name = format!("mordee_twilio_video_{}", appointment_id);
        let identity = format!(
            "{}_{}_{}",
            role_name, user_id.account_id, user_id.user_profile_id
        );

        match &consultation_session.session_data {
            Some(session_data) => match consultation_session.session_provider_name {
                DbMeetingProvider::Twilio => {
                    let deserialized: Option<SessionData> =
                        serde_json::from_value(session_data.0.clone()).ok();
                    let chat_service_sid =
                        if let Some(SessionData::Twilio(twilio_info)) = deserialized.as_ref() {
                            if !twilio_info.session_chat_service_id.is_empty() {
                                Some(twilio_info.session_chat_service_id.clone())
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                    let token = self
                        .twilio_client
                        .create_access_token(
                            room_name.clone(),
                            chat_service_sid,
                            identity.clone(),
                            Some(consultation_session.consultation_end_time),
                        )
                        .await
                        .map_err(|e| SessionError::TwilioError(e.to_string()))?;

                    let session_chat_name = match deserialized.as_ref() {
                        Some(SessionData::Twilio(twilio_info))
                            if !twilio_info.session_chat_id.is_empty() =>
                        {
                            // The chat conversation is created once (by whoever opened the
                            // session first); make sure the requesting participant is also
                            // joined to it. `join_conversation` is idempotent — Twilio returns
                            // 409 for an existing participant and the client treats it as success.
                            // The participant identity MUST match the token identity, otherwise
                            // the client connects with an identity that is not a member and
                            // cannot access the conversation.
                            self.twilio_client
                                .join_conversation(
                                    twilio_info.session_chat_id.clone(),
                                    identity.clone(),
                                )
                                .await?;
                            Some(format!("mordee_twilio_chat_{}", appointment_id))
                        }
                        _ => None,
                    };

                    Ok((
                        ProviderSessionInfo::Twilio(TwilioSessionInfo {
                            session_name: format!("mordee_twilio_video_{}", appointment_id),
                            session_chat_name,
                            session_token: token,
                        }),
                        false,
                    ))
                }
                DbMeetingProvider::TokBox => Err(SessionError::InvalidSessionStatus),
            },
            None => match consultation_session.session_provider_name {
                DbMeetingProvider::Twilio => {
                    // Chat needs no A/V room; video & voice each create their own.
                    let room = match consultation_session.consultation_channel {
                        ConsultationChannelEnum::Chat => None,
                        ConsultationChannelEnum::Voice => Some(
                            self.twilio_client
                                .create_voice_room(room_name.clone(), true)
                                .await
                                .map_err(|e| SessionError::TwilioError(e.to_string()))?,
                        ),
                        ConsultationChannelEnum::Video => Some(
                            self.twilio_client
                                .create_video_room(room_name.clone(), true)
                                .await
                                .map_err(|e| SessionError::TwilioError(e.to_string()))?,
                        ),
                    };

                    let chat_name = format!("mordee_twilio_chat_{}", appointment_id);
                    let conversation = self
                        .twilio_client
                        .create_conversation(chat_name.clone())
                        .await?;

                    // Participant identity MUST match the token identity (see above),
                    // otherwise the client cannot access the conversation it joins.
                    self.twilio_client
                        .join_conversation(conversation.sid.clone(), identity.clone())
                        .await?;

                    let session_wrap = CreateTwilioSessionWrap {
                        room,
                        conversation: Some((
                            conversation.sid.clone(),
                            conversation.chat_service_sid.clone(),
                        )),
                    };

                    let session_created = self
                        .repo
                        .init_session_data(appointment_id, session_wrap.into())
                        .await?;

                    let token = self
                        .twilio_client
                        .create_access_token(
                            room_name.clone(),
                            Some(conversation.chat_service_sid.clone()),
                            identity.clone(),
                            Some(consultation_session.consultation_end_time),
                        )
                        .await
                        .map_err(|e| SessionError::TwilioError(e.to_string()))?;

                    Ok((
                        ProviderSessionInfo::Twilio(TwilioSessionInfo {
                            session_name: format!("mordee_twilio_video_{}", appointment_id),
                            session_chat_name: Some(chat_name),
                            session_token: token,
                        }),
                        session_created,
                    ))
                }
                DbMeetingProvider::TokBox => Err(GetSessionInfoResult::SessionNotFound.into()),
            },
        }
    }

    async fn publish_session_lifecycle_events(
        &self,
        user_id: &UserIdentity,
        appointment_id: &str,
        consultation_session: &DbConsultationSession,
        session_created: bool,
    ) -> Result<(), SessionError> {
        let session_details = self
            .repo
            .get_session_details(appointment_id)
            .await?
            .ok_or(GetSessionInfoResult::SessionNotFound)?;

        let patient_identity = patient_identity_from_session_details(&session_details);
        let joined_at = jiff::Timestamp::now().as_second();
        let participant_role = SessionParticipantRole::from(user_id.account_type.clone());
        let session_participant = session_participant_from_role(participant_role);

        if session_created {
            let message = SessionMessage::SessionCreated {
                booking_id: session_details.booking_id.clone(),
                patient_identity: patient_identity.clone(),
                doctor_id: session_details.doctor_id,
                session_provider: session_provider_name(consultation_session.session_provider_name),
                session_initiator: session_participant.clone(),
                consultation_start_time: consultation_session.consultation_start_time,
                consultation_duration_in_second: (consultation_session.consultation_end_time
                    - consultation_session.consultation_start_time)
                    .try_into()
                    .unwrap_or(0),
                created_at: joined_at,
            };

            self.event_publisher
                .publish_consultation_event(ConsultationEvent::SessionMessage(message))
                .await
                .map_err(|e| anyhow::anyhow!("Failed to publish session created event: {e}"))?;
        }

        let join_record = self
            .repo
            .mark_participant_joined(appointment_id, participant_role, joined_at)
            .await?;

        self.publish_join_events(
            participant_role,
            &session_details,
            patient_identity,
            joined_at,
            join_record,
        )
        .await
    }

    async fn publish_join_events(
        &self,
        participant_role: SessionParticipantRole,
        session_details: &SessionDetails,
        patient_identity: PartialUserIdentity,
        joined_at: i64,
        join_record: ParticipantJoinRecord,
    ) -> Result<(), SessionError> {
        if join_record.participant_joined_first_time {
            let message = match participant_role {
                SessionParticipantRole::Patient => SessionMessage::PatientJoined {
                    booking_id: session_details.booking_id.clone(),
                    patient_identity: patient_identity.clone(),
                    doctor_id: session_details.doctor_id,
                    joined_at,
                },
                SessionParticipantRole::Doctor => SessionMessage::DoctorJoined {
                    booking_id: session_details.booking_id.clone(),
                    patient_identity: patient_identity.clone(),
                    doctor_id: session_details.doctor_id,
                    joined_at,
                },
            };

            self.event_publisher
                .publish_consultation_event(ConsultationEvent::SessionMessage(message))
                .await
                .map_err(|e| anyhow::anyhow!("Failed to publish participant joined event: {e}"))?;
        }

        if join_record.all_participants_joined_first_time {
            let (Some(patient_joined_at), Some(doctor_joined_at)) =
                (join_record.patient_joined_at, join_record.doctor_joined_at)
            else {
                tracing::warn!(
                    appointment_id = %session_details.appointment_id,
                    "all participants joined flag returned without both timestamps"
                );
                return Ok(());
            };

            let message = SessionMessage::AllParticipantJoined {
                booking_id: session_details.booking_id.clone(),
                patient_identity,
                doctor_id: session_details.doctor_id,
                patient_joined_at,
                doctor_joined_at,
            };

            self.event_publisher
                .publish_consultation_event(ConsultationEvent::SessionMessage(message))
                .await
                .map_err(|e| {
                    anyhow::anyhow!("Failed to publish all participants joined event: {e}")
                })?;
        }

        Ok(())
    }

    fn is_authorized(&self, user_id: &UserIdentity, session: &DbConsultationSession) -> bool {
        let is_patient = user_id.account_type == AccountType::Patient
            && user_id.user_profile_id == session.patient_profile_id as u64;

        let is_doctor = user_id.account_type == AccountType::Doctor
            && user_id.user_profile_id == session.doctor_profile_id as u64;

        is_patient || is_doctor
    }

    fn is_within_acceptable_time(&self, start_time: i64, end_time: i64, now: i64) -> bool {
        let lead_time = self.config.lead_time_duration();
        let acceptable_start = start_time - lead_time.as_secs() as i64;
        now >= acceptable_start && now < end_time
    }
}

fn patient_identity_from_session_details(session_details: &SessionDetails) -> PartialUserIdentity {
    PartialUserIdentity {
        account_id: session_details.patient_account_id,
        user_profile_id: session_details.patient_profile_id,
        tenant_id: session_details.tenant_id,
        oidc_user_id: None,
    }
}

fn session_provider_name(provider: DbMeetingProvider) -> String {
    match provider {
        DbMeetingProvider::Twilio => "TWILIO".to_string(),
        DbMeetingProvider::TokBox => "TOKBOX".to_string(),
    }
}

fn session_participant_from_role(role: SessionParticipantRole) -> SessionParticipant {
    match role {
        SessionParticipantRole::Patient => SessionParticipant::Patient,
        SessionParticipantRole::Doctor => SessionParticipant::Doctor,
    }
}
