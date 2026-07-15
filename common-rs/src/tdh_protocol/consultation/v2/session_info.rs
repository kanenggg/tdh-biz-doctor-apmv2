use serde::{Deserialize, Serialize};

use crate::tdh_protocol::{
    common::meeting_provider::MeetingProvider, consultation::channel::ConsultationChannel,
};
//
// #[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
// #[derive(Debug, Clone, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub enum StartingType {
//     StartLater,
//     StartNow,
// }

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum SessionStatus {
    RoomCreated,
    Started,
    Ended,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type")]
pub enum GetSessionInfoResult {
    #[serde(rename = "GetSessionInfoResult.SessionReady")]
    SessionReady(SessionReady),
    #[serde(rename = "GetSessionInfoResult.SessionNotFound")]
    SessionNotFound,
    #[serde(rename = "GetSessionInfoResult.SessionIsFinished")]
    SessionIsFinished,
    #[serde(rename = "GetSessionInfoResult.SessionIsNotReady")]
    SessionIsNotReady,
    #[serde(rename = "GetSessionInfoResult.ProviderIsOutOfService")]
    ProviderIsOutOfService(MeetingProvider),
    #[serde(rename = "GetSessionInfoResult.Unauthorized")]
    Unauthorized,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum ProviderSessionInfo {
    TokBox(TokBoxSessionInfo),
    Twilio(TwilioSessionInfo),
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokBoxSessionInfo {
    pub conference_provider_id: i32,
    pub session_id: String,
    pub session_token: String,
    pub appointment_no: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwilioSessionInfo {
    pub session_name: String,
    pub session_chat_name: Option<String>,
    pub session_token: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionReady {
    pub session_info: ProviderSessionInfo,
    pub session_start_time: i64,
    pub session_end_time: i64,
    pub is_facial_verified: bool,
    pub is_required_patient_verification: Option<bool>,
    pub session_channel: ConsultationChannel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtdb_access: Option<RtdbAccess>,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RtdbAccess {
    /// Firebase custom token. The client exchanges it through Firebase Auth;
    /// it is never an RTDB database-admin credential.
    pub custom_token: String,
    pub expires_at: i64,
    pub path: String,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionProviderNotSupport {
    pub privider_name: MeetingProvider,
}
