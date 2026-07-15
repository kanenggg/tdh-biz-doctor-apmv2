use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ConsultationChannel {
    #[serde(rename = "video")]
    Video,
    #[serde(rename = "voice")]
    Voice,
    #[serde(rename = "chat")]
    Chat,
}

impl From<ConsultationChannel> for String {
    fn from(val: ConsultationChannel) -> Self {
        match val {
            ConsultationChannel::Video => "video".to_string(),
            ConsultationChannel::Voice => "voice".to_string(),
            ConsultationChannel::Chat => "chat".to_string(),
        }
    }
}
