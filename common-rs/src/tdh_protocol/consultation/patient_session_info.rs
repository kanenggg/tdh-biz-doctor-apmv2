use serde::{Deserialize, Serialize};

use super::wrapped_session_info::WrappedSessionInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "camelCase")]
pub enum GetPatientSessionInfoResult {
    #[serde(rename = "GetPatientSessionInfo.SessionInfo")]
    SessionInfo(WrappedSessionInfo),
    #[serde(rename = "GetPatientSessionInfo.SessionNotFound")]
    SessionNotFound,
    #[serde(rename = "GetPatientSessionInfo.SessionIsFinished")]
    SessionIsFinished,
    #[serde(rename = "GetPatientSessionInfo.SessionIsNotReady")]
    SessionIsNotReady,
    #[serde(rename = "GetPatientSessionInfo.Unauthorized")]
    Unauthorized,
}
