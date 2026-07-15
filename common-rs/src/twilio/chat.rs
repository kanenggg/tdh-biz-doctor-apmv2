use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateConversationRequest {
    #[serde(rename = "UniqueName")]
    pub unique_name: String,
    #[serde(rename = "Timers.Closed", skip_serializing_if = "Option::is_none")]
    pub timers_closed: Option<String>,
}

impl CreateConversationRequest {
    pub fn new(unique_name: String) -> Self {
        CreateConversationRequest {
            unique_name,
            timers_closed: Some("PT1H".to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateConversationResponse {
    pub sid: String,
    pub account_sid: String,
    pub chat_service_sid: String,
    pub messaging_service_sid: String,
    pub friendly_name: Option<String>,
    pub unique_name: String,
    pub attributes: String,
    pub date_created: String,
    pub date_updated: String,
    pub state: String,
    pub timers: Option<Timers>,
    pub bindings: Option<String>,
    pub url: String,
    pub links: ConversationLinks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timers {
    pub date_inactive: Option<String>,
    pub date_closed: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationLinks {
    pub participants: String,
    pub messages: String,
    pub webhooks: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinConversationRequest {
    pub identity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinConversationResponse {
    pub sid: String,
    pub account_sid: String,
    pub conversation_sid: String,
    pub identity: String,
    pub attributes: String,
    pub messaging_binding: Option<MessagingBinding>,
    pub role_sid: String,
    pub date_created: String,
    pub date_updated: String,
    pub url: String,
    pub last_read_message_index: Option<String>,
    pub last_read_message_time_stamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagingBinding {
    #[serde(rename = "type")]
    pub binding_type: String,
    pub address: String,
    pub proxy_address: String,
}
