use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeetingProvider {
    #[serde(rename = "Twilio")]
    Twilio,
    #[serde(rename = "TokBox")]
    TokBox,
}

impl Display for MeetingProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MeetingProvider::Twilio => write!(f, "Twilio"),
            MeetingProvider::TokBox => write!(f, "TokBox"),
        }
    }
}
