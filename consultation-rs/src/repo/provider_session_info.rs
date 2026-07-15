use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "camelCase")]
pub enum SessionData {
    TokBox(TokBoxSessionInfo),
    Twilio(TwilioSessionInfo),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokBoxSessionInfo {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwilioSessionInfo {
    pub recording_url: String,
    pub session_chat_id: String,
    pub chat_recording_url: String,
    pub session_chat_service_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_room_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_room_name: Option<String>,
}
