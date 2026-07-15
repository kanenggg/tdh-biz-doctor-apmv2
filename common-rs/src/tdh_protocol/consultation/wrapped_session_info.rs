use crate::tdh_protocol::common::SessionInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WrappedSessionInfo {
    pub session_info: SessionInfo,
}
