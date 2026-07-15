use super::EventPublisher;
use crate::common::tdh_protocol::consultation::{
    ConsultationEvent, PostSessionMessage, PreSessionMessage, SessionMessage,
};
use crate::doctor_timeslot::configuration_events::model::DoctorTimeslotConfigChangedEvent;
use google_cloud_googleapis::pubsub::v1::PubsubMessage;
use google_cloud_pubsub::client::{Client, ClientConfig};
use google_cloud_pubsub::publisher::Publisher;
use uuid::Uuid;

const EVENT_TYPE_ATTRIBUTE: &str = "event_type";
const EVENT_ID_ATTRIBUTE: &str = "event_id";

/// Real GCP PubSub publisher.
///
/// Set `PUBSUB_EMULATOR_HOST=localhost:8085` (or pass via `pubsub_emulator_host` config) to
/// use the local emulator instead of production GCP.
#[derive(Clone)]
pub struct PubSubEventPublisher {
    client: Client,
    consultation_publisher: Publisher,
}

impl PubSubEventPublisher {
    pub async fn new(
        project_id: &str,
        consultation_topic: &str,
        emulator_host: Option<&str>,
    ) -> Result<Self, anyhow::Error> {
        if let Some(host) = emulator_host {
            if !host.is_empty() {
                // SAFETY: Called once at startup before other threads read env vars.
                unsafe {
                    std::env::set_var("PUBSUB_EMULATOR_HOST", host);
                }
            }
        }

        let mut config = ClientConfig::default().with_auth().await?;
        config.project_id = Some(pubsub_project_id(project_id, config.project_id.take())?);
        let client = Client::new(config).await?;

        if consultation_topic.trim().is_empty() {
            anyhow::bail!("booking.pubsub_topic is required")
        }
        let consultation_publisher = client.topic(consultation_topic).new_publisher(None);

        Ok(Self {
            client,
            consultation_publisher,
        })
    }
}

fn pubsub_project_id(
    configured_project_id: &str,
    discovered_project_id: Option<String>,
) -> Result<String, anyhow::Error> {
    let project_id = configured_project_id.trim();
    if !project_id.is_empty() {
        return Ok(project_id.to_string());
    }

    if let Some(project_id) = discovered_project_id {
        let project_id = project_id.trim();
        if !project_id.is_empty() {
            return Ok(project_id.to_string());
        }
    }

    anyhow::bail!("google_cloud.project_id is required for Pub/Sub publisher")
}

#[async_trait::async_trait]
impl EventPublisher for PubSubEventPublisher {
    async fn publish_consultation_event(
        &self,
        event: ConsultationEvent,
    ) -> Result<(), anyhow::Error> {
        let msg = consultation_event_pubsub_message(&event, None)?;
        publish_message(&self.consultation_publisher, msg).await
    }

    async fn publish_consultation_event_with_id(
        &self,
        event_id: Uuid,
        event: ConsultationEvent,
    ) -> Result<(), anyhow::Error> {
        let msg = consultation_event_pubsub_message(&event, Some(event_id))?;
        publish_message(&self.consultation_publisher, msg).await
    }

    async fn publish_doctor_timeslot_config_changed_event(
        &self,
        event: DoctorTimeslotConfigChangedEvent,
    ) -> Result<(), anyhow::Error> {
        let topic = self.client.topic(event.topic());
        let publisher = topic.new_publisher(None);
        let msg = doctor_timeslot_config_changed_pubsub_message(&event)?;
        publish_message(&publisher, msg).await
    }
}

fn session_message_pubsub_message(event: &SessionMessage) -> Result<PubsubMessage, anyhow::Error> {
    pubsub_message(event, session_message_event_type(event), None)
}

fn consultation_event_pubsub_message(
    event: &ConsultationEvent,
    event_id: Option<Uuid>,
) -> Result<PubsubMessage, anyhow::Error> {
    pubsub_message(event, consultation_event_type(event), event_id)
}

fn doctor_timeslot_config_changed_pubsub_message(
    event: &DoctorTimeslotConfigChangedEvent,
) -> Result<PubsubMessage, anyhow::Error> {
    pubsub_message(event, event.event_type.as_str(), None)
}

async fn publish_message(publisher: &Publisher, msg: PubsubMessage) -> Result<(), anyhow::Error> {
    let awaiter = publisher.publish(msg).await;
    awaiter
        .get()
        .await
        .map_err(|e| anyhow::anyhow!("PubSub publish failed: {}", e))?;
    Ok(())
}

fn pubsub_message<T: serde::Serialize>(
    event: &T,
    event_type: &'static str,
    event_id: Option<Uuid>,
) -> Result<PubsubMessage, anyhow::Error> {
    let mut attributes = std::collections::HashMap::from([(
        EVENT_TYPE_ATTRIBUTE.to_string(),
        event_type.to_string(),
    )]);
    if let Some(event_id) = event_id {
        attributes.insert(EVENT_ID_ATTRIBUTE.to_string(), event_id.to_string());
    }
    Ok(PubsubMessage {
        data: serde_json::to_vec(event)?,
        attributes,
        ..Default::default()
    })
}

fn consultation_event_type(event: &ConsultationEvent) -> &'static str {
    match event {
        ConsultationEvent::PreSessionMessage(event) => pre_session_message_event_type(event),
        ConsultationEvent::SessionMessage(event) => session_message_event_type(event),
        ConsultationEvent::PostSessionMessage(event) => post_session_message_event_type(event),
    }
}

fn pre_session_message_event_type(event: &PreSessionMessage) -> &'static str {
    match event {
        PreSessionMessage::TimeslotReserved { .. } => "TimeslotReserved",
        PreSessionMessage::ReservationCancelled { .. } => "ReservationCancelled",
        PreSessionMessage::ReservationExpired { .. } => "ReservationExpired",
        PreSessionMessage::ConsultationBooked { .. } => "ConsultationBooked",
        PreSessionMessage::ConsultationCancelled { .. } => "ConsultationCancelled",
    }
}

fn session_message_event_type(event: &SessionMessage) -> &'static str {
    match event {
        SessionMessage::SessionCreated { .. } => "SessionCreated",
        SessionMessage::PatientJoined { .. } => "PatientJoined",
        SessionMessage::DoctorJoined { .. } => "DoctorJoined",
        SessionMessage::AllParticipantJoined { .. } => "AllParticipantJoined",
        SessionMessage::PatientDisconnected { .. } => "PatientDisconnected",
        SessionMessage::DoctorDisconnected { .. } => "DoctorDisconnected",
        SessionMessage::SessionTerminated { .. } => "SessionTerminated",
    }
}

fn post_session_message_event_type(event: &PostSessionMessage) -> &'static str {
    match event {
        PostSessionMessage::ConsultationSummarized { .. } => "ConsultationSummarized",
        PostSessionMessage::FollowUpRequired { .. } => "FollowUpRequired",
        PostSessionMessage::FollowUpRequestExpired { .. } => "FollowUpRequestExpired",
        PostSessionMessage::PatientAcceptedFollowUp { .. } => "PatientAcceptedFollowUp",
        PostSessionMessage::FollowUpCancelled { .. } => "FollowUpCancelled",
    }
}

#[cfg(test)]
pub struct MockEventPublisher;

#[cfg(test)]
#[async_trait::async_trait]
impl EventPublisher for MockEventPublisher {
    async fn publish_consultation_event(
        &self,
        _event: ConsultationEvent,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn publish_doctor_timeslot_config_changed_event(
        &self,
        _event: DoctorTimeslotConfigChangedEvent,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

pub struct NoOpEventPublisher;

#[async_trait::async_trait]
impl EventPublisher for NoOpEventPublisher {
    async fn publish_consultation_event(
        &self,
        _event: ConsultationEvent,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn publish_doctor_timeslot_config_changed_event(
        &self,
        _event: DoctorTimeslotConfigChangedEvent,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::tdh_protocol::common::PartialUserIdentity;
    use crate::common::tdh_protocol::consultation::{
        ConsultationChannel, PreSessionMessage, SessionParticipant, TerminationCode,
    };
    use crate::consultation_config::model::DoctorConfigIdentity;
    use crate::doctor_timeslot::configuration_events::model::DoctorTimeslotConfigChangeType;
    use uuid::Uuid;

    fn patient_identity() -> PartialUserIdentity {
        PartialUserIdentity {
            account_id: 1,
            user_profile_id: 2,
            tenant_id: 3,
            oidc_user_id: None,
        }
    }

    #[test]
    fn session_message_has_event_type_attribute() {
        let event = SessionMessage::SessionTerminated {
            booking_id: "booking-1".to_string(),
            patient_identity: patient_identity(),
            doctor_id: 4,
            termination_code: TerminationCode::BothPartiesAbsent,
            terminated_by: SessionParticipant::System,
            terminated_at: 5,
        };

        let msg = session_message_pubsub_message(&event).expect("message should serialize");

        assert_eq!(
            msg.attributes.get("event_type").map(String::as_str),
            Some("SessionTerminated")
        );
        assert_eq!(msg.data, serde_json::to_vec(&event).unwrap());
    }

    #[test]
    fn consultation_event_has_inner_event_type_attribute() {
        let event = ConsultationEvent::PreSessionMessage(PreSessionMessage::TimeslotReserved {
            booking_id: "booking-1".to_string(),
            patient_identity: patient_identity(),
            doctor_id: 4,
            biz_unit_id: 5,
            reserved_from: 6,
            reservation_duration_sec: 7,
            consultation_channel: ConsultationChannel::Video,
            reserved_at: 8,
        });

        let msg =
            consultation_event_pubsub_message(&event, None).expect("message should serialize");

        assert_eq!(
            msg.attributes.get("event_type").map(String::as_str),
            Some("TimeslotReserved")
        );
        assert_eq!(msg.data, serde_json::to_vec(&event).unwrap());
    }

    #[test]
    fn doctor_timeslot_config_event_has_event_type_attribute() {
        let event = DoctorTimeslotConfigChangedEvent::new(
            DoctorConfigIdentity {
                doctor_id: Uuid::nil(),
                doctor_account_id: 1,
                doctor_profile_id: 2,
            },
            DoctorTimeslotConfigChangeType::TimeslotConfiguration,
            Some(true),
        );

        let msg = doctor_timeslot_config_changed_pubsub_message(&event)
            .expect("message should serialize");

        assert_eq!(
            msg.attributes.get("event_type").map(String::as_str),
            Some("DoctorTimeslotConfigurationChanged")
        );
        assert_eq!(msg.data, serde_json::to_vec(&event).unwrap());
    }

    #[test]
    fn pubsub_project_id_uses_configured_value() {
        assert_eq!(
            pubsub_project_id("tdg-dh-truehealth-core-nonprod", None).unwrap(),
            "tdg-dh-truehealth-core-nonprod"
        );
    }

    #[test]
    fn pubsub_project_id_falls_back_to_discovered_value() {
        assert_eq!(
            pubsub_project_id("", Some("local-project".to_string())).unwrap(),
            "local-project"
        );
    }

    #[test]
    fn pubsub_project_id_rejects_blank_value() {
        let error = pubsub_project_id("  ", None).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("google_cloud.project_id is required")
        );
    }
}
