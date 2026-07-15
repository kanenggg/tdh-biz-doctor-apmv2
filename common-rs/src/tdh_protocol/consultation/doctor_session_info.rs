use serde::{Deserialize, Serialize};

use super::wrapped_session_info::WrappedSessionInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokBoxSessionInfo {
    pub conference_provider_id: i32,
    pub session_id: String,
    pub session_token: String,
    pub appointment_no: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwilioSessionInfo {
    pub conference_provider_id: i32,
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_chat_name: Option<String>,
    pub session_token: String,
    pub appointment_no: String,
    pub channel_type: String,
    pub is_facial_verified: bool,
}

#[deprecated(note = "breaking change to `doctor/sessoin_info.rs`")]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "camelCase")]
pub enum GetDoctorSessionInfoResult {
    #[serde(rename = "GetDoctorSessionInfo.TokBoxSessionInfo")]
    TokBoxSessionInfo(TokBoxSessionInfo),
    #[serde(rename = "GetDoctorSessionInfo.TwilioSessionInfo")]
    TwilioSessionInfo(TwilioSessionInfo),
    #[serde(rename = "GetDoctorSessionInfo.SessionInfo")]
    SessionInfo(WrappedSessionInfo),
    #[serde(rename = "GetDoctorSessionInfo.SessionNotFound")]
    SessionNotFound,
    #[serde(rename = "GetDoctorSessionInfo.SessionIsFinished")]
    SessionIsFinished,
    #[serde(rename = "GetDoctorSessionInfo.SessionIsNotReady")]
    SessionIsNotReady,
    #[serde(rename = "GetDoctorSessionInfo.Unauthorized")]
    Unauthorized,
}
