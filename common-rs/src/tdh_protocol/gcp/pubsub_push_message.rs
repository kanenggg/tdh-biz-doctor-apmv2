use base64::{prelude::BASE64_STANDARD, Engine};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct PubSubPushMessage {
    pub message: PubSubMessage,
    pub subscription: String,
}

#[derive(Debug, Deserialize)]
pub struct PubSubMessage {
    pub data: String,
    pub message_id: String,
    pub attributes: Option<HashMap<String, String>>,
}

impl PubSubPushMessage {
    pub fn read_data<T>(&self) -> Result<T, Box<dyn std::error::Error>>
    where
        T: DeserializeOwned,
    {
        let bytes = BASE64_STANDARD.decode(&self.message.data)?;
        let value = serde_json::from_slice(&bytes)?;
        Ok(value)
    }
}
