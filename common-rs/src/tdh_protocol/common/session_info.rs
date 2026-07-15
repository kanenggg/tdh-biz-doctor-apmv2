use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "camelCase")]
pub enum SessionChannel {
    Video {
        session_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_chat_name: Option<String>,
        session_token: String,
    },
    Voice {
        session_name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        session_chat_name: Option<String>,
        session_token: String,
    },
    Chat {
        session_chat_name: String,
        session_token: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "camelCase")]
pub enum StartingType {
    StartNow,
    StartLater,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_channel: SessionChannel,
    pub session_start_time: i64,
    pub session_end_time: i64,
    pub starting_type: StartingType,
    pub is_facial_verified: bool,
    pub appointment_no: String,
}
