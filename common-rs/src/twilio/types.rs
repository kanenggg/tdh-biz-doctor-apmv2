use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TwilioConfig {
    // This allows "baseUrl" (YAML) OR "base_url" (Env)
    #[serde(alias = "base_url")]
    pub base_url: String,

    #[serde(alias = "video_base_url")]
    pub video_base_url: String,

    #[serde(alias = "chat_base_url")]
    pub chat_base_url: String,

    #[serde(alias = "account_sid")]
    pub account_sid: String,

    #[serde(alias = "api_key_sid")]
    pub api_key_sid: String,

    #[serde(alias = "api_key_secret")]
    pub api_key_secret: String,

    #[serde(alias = "auth_token")]
    pub auth_token: String,

    #[serde(alias = "callback_url")]
    pub callback_url: String,
}

impl TwilioConfig {
    pub fn basic_auth(&self) -> String {
        format!("{}:{}", self.account_sid, self.auth_token)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub code: Option<u32>,
    pub message: String,
    pub more_info: Option<String>,
    pub status: Option<u16>,
}
