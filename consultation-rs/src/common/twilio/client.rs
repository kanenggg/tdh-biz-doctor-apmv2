use common_rs::twilio::{
    CreateConversationRequest, CreateConversationResponse, CreateRoomRequest, CreateRoomResponse,
    JoinConversationResponse, TwilioAccessTokenBuilder, TwilioConfig,
};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use thiserror::Error;

// ─── Error ────────────────────────────────────────────────────────────────────

#[derive(Error, Debug)]
pub enum TwilioError {
    #[error("HTTP error: status {status_code}, message: {message}")]
    HttpError { status_code: u16, message: String },
    #[error("Decode failure: {0}")]
    DecodeFailure(String),
    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("JWT error: {0}")]
    JwtError(String),
}

impl From<common_rs::twilio::JwtError> for TwilioError {
    fn from(e: common_rs::twilio::JwtError) -> Self {
        TwilioError::JwtError(e.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct TwilioErrorResponse {
    code: i32,
    message: String,
    more_info: Option<String>,
    status: u16,
}

// ─── Trait ────────────────────────────────────────────────────────────────────

/// Abstraction over Twilio API calls; enables test mocking.
#[async_trait::async_trait]
pub trait TwilioClient: Send + Sync {
    async fn create_video_room(
        &self,
        name: String,
        record: bool,
    ) -> Result<CreateRoomResponse, TwilioError>;

    async fn fetch_room(&self, name: String) -> Result<CreateRoomResponse, TwilioError>;

    async fn complete_room(&self, room_sid: String) -> Result<(), TwilioError>;

    async fn create_voice_room(
        &self,
        name: String,
        record: bool,
    ) -> Result<CreateRoomResponse, TwilioError>;

    async fn create_conversation(
        &self,
        name: String,
    ) -> Result<CreateConversationResponse, TwilioError>;

    async fn join_conversation(
        &self,
        conversation_sid: String,
        identity: String,
    ) -> Result<(), TwilioError>;

    async fn close_conversation(&self, conversation_sid: String) -> Result<(), TwilioError>;

    async fn create_access_token(
        &self,
        room_name: String,
        chat_service_sid: Option<String>,
        identity: String,
        expires_at: Option<i64>,
    ) -> Result<String, common_rs::twilio::JwtError>;
}

// ─── Implementation ───────────────────────────────────────────────────────────

pub struct TwilioClientImpl {
    config: TwilioConfig,
    client: Client,
    token_builder: TwilioAccessTokenBuilder,
}

impl TwilioClientImpl {
    pub fn new(config: TwilioConfig) -> Self {
        let client = Client::new();
        let token_builder = TwilioAccessTokenBuilder::new(
            config.account_sid.clone(),
            config.api_key_sid.clone(),
            config.api_key_secret.clone(),
        );
        Self {
            config,
            client,
            token_builder,
        }
    }
}

#[async_trait::async_trait]
impl TwilioClient for TwilioClientImpl {
    async fn create_video_room(
        &self,
        name: String,
        record: bool,
    ) -> Result<CreateRoomResponse, TwilioError> {
        let callback_url = self.config.callback_url.clone();
        let request = CreateRoomRequest::new(name, callback_url, record, "group".to_string());
        create_room(request, &self.config, &self.client).await
    }

    async fn fetch_room(&self, name: String) -> Result<CreateRoomResponse, TwilioError> {
        fetch_room(&name, &self.config, &self.client).await
    }

    async fn complete_room(&self, room_sid: String) -> Result<(), TwilioError> {
        complete_room(&room_sid, &self.config, &self.client).await
    }

    async fn create_voice_room(
        &self,
        name: String,
        record: bool,
    ) -> Result<CreateRoomResponse, TwilioError> {
        let callback_url = self.config.callback_url.clone();
        let request = CreateRoomRequest::voice(name, callback_url, record);
        create_room(request, &self.config, &self.client).await
    }

    async fn create_conversation(
        &self,
        name: String,
    ) -> Result<CreateConversationResponse, TwilioError> {
        let request = CreateConversationRequest::new(name);
        create_conversation(request, &self.config, &self.client).await
    }

    async fn join_conversation(
        &self,
        conversation_sid: String,
        identity: String,
    ) -> Result<(), TwilioError> {
        join_conversation(&conversation_sid, &identity, &self.config, &self.client).await?;
        Ok(())
    }

    async fn close_conversation(&self, conversation_sid: String) -> Result<(), TwilioError> {
        close_conversation(&conversation_sid, &self.config, &self.client).await
    }

    async fn create_access_token(
        &self,
        room_name: String,
        chat_service_sid: Option<String>,
        identity: String,
        expires_at: Option<i64>,
    ) -> Result<String, common_rs::twilio::JwtError> {
        self.token_builder.build_video_chat_token_with_exp(
            &room_name,
            chat_service_sid.as_deref(),
            &identity,
            expires_at,
        )
    }
}

// ─── HTTP helpers (owned here, use reqwest 0.13) ─────────────────────────────

async fn fetch_room(
    unique_name: &str,
    config: &TwilioConfig,
    client: &Client,
) -> Result<CreateRoomResponse, TwilioError> {
    let url = format!("{}/v1/Rooms/{}", config.video_base_url, unique_name);
    let response = client
        .get(&url)
        .basic_auth(&config.account_sid, Some(&config.auth_token))
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    match status.as_u16() {
        200..=299 => {
            serde_json::from_str(&text).map_err(|e| TwilioError::DecodeFailure(e.to_string()))
        }
        _ => Err(TwilioError::HttpError {
            status_code: status.as_u16(),
            message: text,
        }),
    }
}

async fn create_room(
    request: CreateRoomRequest,
    config: &TwilioConfig,
    client: &Client,
) -> Result<CreateRoomResponse, TwilioError> {
    let mut form_data = HashMap::new();
    form_data.insert(
        "RecordParticipantsOnConnect",
        request.record_participants_on_connect.to_string(),
    );
    form_data.insert("StatusCallback", request.status_callback.clone());
    form_data.insert("Type", request.room_type.clone());
    form_data.insert("UniqueName", request.unique_name.clone());
    if let Some(t) = request.empty_room_timeout {
        form_data.insert("EmptyRoomTimeout", t.to_string());
    }
    if let Some(t) = request.unused_room_timeout {
        form_data.insert("UnusedRoomTimeout", t.to_string());
    }
    form_data.insert("AudioOnly", request.audio_only.to_string());

    let url = format!("{}/v1/Rooms", config.video_base_url);
    let response = client
        .post(url)
        .basic_auth(&config.account_sid, Some(&config.auth_token))
        .form(&form_data)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    match status.as_u16() {
        200..=299 | 409 => {
            serde_json::from_str(&text).map_err(|e| TwilioError::DecodeFailure(e.to_string()))
        }
        400 => {
            if let Ok(error_resp) = serde_json::from_str::<TwilioErrorResponse>(&text) {
                if error_resp.code == 53113 {
                    return fetch_room(&request.unique_name, config, client).await;
                }
            }
            Err(TwilioError::HttpError {
                status_code: status.as_u16(),
                message: text,
            })
        }
        _ => Err(TwilioError::HttpError {
            status_code: status.as_u16(),
            message: text,
        }),
    }
}

pub async fn complete_room(
    room_id: &str,
    config: &TwilioConfig,
    client: &Client,
) -> Result<(), TwilioError> {
    let url = format!("{}/v1/Rooms/{}", config.video_base_url, room_id);
    let mut form_data = HashMap::new();
    form_data.insert("Status", "completed".to_string());

    let response = client
        .post(&url)
        .basic_auth(&config.account_sid, Some(&config.auth_token))
        .form(&form_data)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    match status.as_u16() {
        200..=299 => Ok(()),
        _ => Err(TwilioError::HttpError {
            status_code: status.as_u16(),
            message: text,
        }),
    }
}

async fn create_conversation(
    request: CreateConversationRequest,
    config: &TwilioConfig,
    client: &Client,
) -> Result<CreateConversationResponse, TwilioError> {
    let mut form_data = HashMap::new();
    form_data.insert("UniqueName", request.unique_name.clone());
    if let Some(timers) = request.timers_closed.clone() {
        form_data.insert("Timers.Closed", timers);
    }

    let url = format!("{}/v1/Conversations", config.chat_base_url);
    let response = client
        .post(url)
        .basic_auth(&config.account_sid, Some(&config.auth_token))
        .form(&form_data)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    match status.as_u16() {
        200..=299 => {
            serde_json::from_str(&text).map_err(|e| TwilioError::DecodeFailure(e.to_string()))
        }
        409 => fetch_conversation(&request.unique_name, config, client).await,
        _ => Err(TwilioError::HttpError {
            status_code: status.as_u16(),
            message: text,
        }),
    }
}

async fn fetch_conversation(
    unique_name: &str,
    config: &TwilioConfig,
    client: &Client,
) -> Result<CreateConversationResponse, TwilioError> {
    let url = format!("{}/v1/Conversations/{}", config.chat_base_url, unique_name);
    let response = client
        .get(&url)
        .basic_auth(&config.account_sid, Some(&config.auth_token))
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    match status.as_u16() {
        200..=299 => {
            serde_json::from_str(&text).map_err(|e| TwilioError::DecodeFailure(e.to_string()))
        }
        _ => Err(TwilioError::HttpError {
            status_code: status.as_u16(),
            message: text,
        }),
    }
}

async fn join_conversation(
    conversation_sid: &str,
    identity: &str,
    config: &TwilioConfig,
    client: &Client,
) -> Result<JoinConversationResponse, TwilioError> {
    let url = format!(
        "{}/v1/Conversations/{}/Participants",
        config.chat_base_url, conversation_sid
    );
    let mut form_data = HashMap::new();
    form_data.insert("Identity", identity.to_string());

    let response = client
        .post(&url)
        .basic_auth(&config.account_sid, Some(&config.auth_token))
        .form(&form_data)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    match status.as_u16() {
        200..=299 => {
            serde_json::from_str(&text).map_err(|e| TwilioError::DecodeFailure(e.to_string()))
        }
        409 => fetch_participant(conversation_sid, identity, config, client).await,
        _ => Err(TwilioError::HttpError {
            status_code: status.as_u16(),
            message: text,
        }),
    }
}

async fn fetch_participant(
    conversation_sid: &str,
    identity: &str,
    config: &TwilioConfig,
    client: &Client,
) -> Result<JoinConversationResponse, TwilioError> {
    let url = format!(
        "{}/v1/Conversations/{}/Participants/{}",
        config.chat_base_url, conversation_sid, identity
    );
    let response = client
        .get(&url)
        .basic_auth(&config.account_sid, Some(&config.auth_token))
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    match status.as_u16() {
        200..=299 => {
            serde_json::from_str(&text).map_err(|e| TwilioError::DecodeFailure(e.to_string()))
        }
        _ => Err(TwilioError::HttpError {
            status_code: status.as_u16(),
            message: text,
        }),
    }
}

pub async fn close_conversation(
    conversation_id: &str,
    config: &TwilioConfig,
    client: &Client,
) -> Result<(), TwilioError> {
    let url = format!(
        "{}/v1/Conversations/{}",
        config.chat_base_url, conversation_id
    );
    let mut form_data = HashMap::new();
    form_data.insert("State", "closed".to_string());

    let response = client
        .post(&url)
        .basic_auth(&config.account_sid, Some(&config.auth_token))
        .form(&form_data)
        .send()
        .await?;

    let status = response.status();
    let text = response.text().await?;

    match status.as_u16() {
        200..=299 => Ok(()),
        400 => {
            if let Ok(error_resp) = serde_json::from_str::<TwilioErrorResponse>(&text) {
                if error_resp.code == 50377 {
                    tracing::debug!(
                        "Conversation already closed. conversation_id: {}, error: {}",
                        conversation_id,
                        error_resp.message
                    );
                    return Ok(());
                }
            }
            Err(TwilioError::HttpError {
                status_code: status.as_u16(),
                message: text,
            })
        }
        _ => Err(TwilioError::HttpError {
            status_code: status.as_u16(),
            message: text,
        }),
    }
}
