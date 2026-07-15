use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ToSchema, Serialize, Deserialize)]
pub enum ConsultationChannel {
    #[serde(rename = "video")]
    Video,
    #[serde(rename = "chat")]
    Chat,
    #[serde(rename = "voice")]
    Voice,
}
