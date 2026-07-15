use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct TwilioStatusCallback {
    #[serde(alias = "StatusCallbackEvent", alias = "statusCallbackEvent")]
    pub status_callback_event: Option<String>,
    #[serde(alias = "RoomName", alias = "roomName")]
    pub room_name: Option<String>,
    #[serde(alias = "RoomSid", alias = "roomSid")]
    pub room_sid: Option<String>,
    #[serde(alias = "ParticipantIdentity", alias = "participantIdentity")]
    pub participant_identity: Option<String>,
    #[serde(alias = "ParticipantStatus", alias = "participantStatus")]
    pub participant_status: Option<String>,
    #[serde(alias = "Timestamp", alias = "timestamp")]
    pub timestamp: Option<String>,
    #[serde(alias = "SequenceNumber", alias = "sequenceNumber")]
    pub sequence_number: Option<String>,
}

impl TwilioStatusCallback {
    pub fn provider_event_id(&self) -> String {
        if let Some(sequence_number) = self.sequence_number.as_ref().filter(|s| !s.is_empty()) {
            return format!(
                "{}:{}:{}",
                self.room_sid.as_deref().unwrap_or_default(),
                self.participant_identity.as_deref().unwrap_or_default(),
                sequence_number
            );
        }

        format!(
            "{}:{}:{}:{}",
            self.room_sid.as_deref().unwrap_or_default(),
            self.room_name.as_deref().unwrap_or_default(),
            self.participant_identity.as_deref().unwrap_or_default(),
            self.status_callback_event.as_deref().unwrap_or_default()
        )
    }

    pub fn is_participant_disconnected(&self) -> bool {
        self.status_callback_event
            .as_deref()
            .map(|event| event.eq_ignore_ascii_case("participant-disconnected"))
            .unwrap_or(false)
            || self
                .participant_status
                .as_deref()
                .map(|status| status.eq_ignore_ascii_case("disconnected"))
                .unwrap_or(false)
    }
}
