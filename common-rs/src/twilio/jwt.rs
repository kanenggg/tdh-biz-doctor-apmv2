use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::Sha256;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JwtError {
    #[error("Encoding error: {0}")]
    EncodingError(String),
    #[error("Missing required field: {0}")]
    MissingField(String),
    #[error("Invalid configuration")]
    InvalidConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoGrant {
    pub room: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatGrant {
    pub service_sid: String,
}

#[derive(Debug, Serialize)]
struct TwilioJwtHeader {
    typ: String,
    alg: String,
    cty: String,
}

pub struct TwilioAccessTokenBuilder {
    account_sid: String,
    api_key_sid: String,
    api_key_secret: String,
}

impl TwilioAccessTokenBuilder {
    pub fn new(account_sid: String, api_key_sid: String, api_key_secret: String) -> Self {
        Self {
            account_sid,
            api_key_sid,
            api_key_secret,
        }
    }

    fn validate(&self) -> Result<(), JwtError> {
        if self.account_sid.is_empty() {
            return Err(JwtError::MissingField("account_sid".to_string()));
        }
        if self.api_key_sid.is_empty() {
            return Err(JwtError::MissingField("api_key_sid".to_string()));
        }
        if self.api_key_secret.is_empty() {
            return Err(JwtError::MissingField("api_key_secret".to_string()));
        }
        Ok(())
    }

    pub fn build_video_token(&self, room_name: &str, identity: &str) -> Result<String, JwtError> {
        self.build_video_token_with_exp(room_name, identity, None)
    }

    pub fn build_video_token_with_exp(
        &self,
        room_name: &str,
        identity: &str,
        expires_at: Option<i64>,
    ) -> Result<String, JwtError> {
        self.validate()?;

        let video_grant = VideoGrant {
            room: room_name.to_string(),
        };

        let grants = serde_json::json!({
            "identity": identity,
            "video": video_grant
        });

        self.build_token_with_exp(grants, expires_at)
    }

    pub fn build_chat_token(&self, service_sid: &str, identity: &str) -> Result<String, JwtError> {
        self.build_chat_token_with_exp(service_sid, identity, None)
    }

    pub fn build_chat_token_with_exp(
        &self,
        service_sid: &str,
        identity: &str,
        expires_at: Option<i64>,
    ) -> Result<String, JwtError> {
        self.validate()?;

        let chat_grant = ChatGrant {
            service_sid: service_sid.to_string(),
        };

        let grants = serde_json::json!({
            "identity": identity,
            "chat": chat_grant
        });

        self.build_token_with_exp(grants, expires_at)
    }

    pub fn build_video_chat_token(
        &self,
        room_name: &str,
        service_sid: Option<&str>,
        identity: &str,
    ) -> Result<String, JwtError> {
        self.build_video_chat_token_with_exp(room_name, service_sid, identity, None)
    }

    pub fn build_video_chat_token_with_exp(
        &self,
        room_name: &str,
        service_sid: Option<&str>,
        identity: &str,
        expires_at: Option<i64>,
    ) -> Result<String, JwtError> {
        self.validate()?;

        let video_grant = VideoGrant {
            room: room_name.to_string(),
        };

        let mut grants_obj = serde_json::json!({
            "identity": identity,
            "video": video_grant
        });

        if let Some(sid) = service_sid {
            let chat_grant = ChatGrant {
                service_sid: sid.to_string(),
            };
            grants_obj["chat"] = serde_json::to_value(chat_grant).unwrap();
        }

        self.build_token_with_exp(grants_obj, expires_at)
    }

    fn build_token(&self, grants: Value) -> Result<String, JwtError> {
        self.build_token_with_exp(grants, None)
    }

    fn build_token_with_exp(
        &self,
        grants: Value,
        expires_at: Option<i64>,
    ) -> Result<String, JwtError> {
        let now = Utc::now();
        let exp = match expires_at {
            Some(exp_time) => {
                let exp_datetime =
                    DateTime::<Utc>::from_timestamp(exp_time, 0).ok_or_else(|| {
                        JwtError::EncodingError("Invalid expiration timestamp".to_string())
                    })?;
                exp_datetime
            }
            None => now + Duration::hours(24),
        };

        let payload = JwtClaims {
            iss: self.api_key_sid.clone(),
            sub: self.account_sid.clone(),
            exp: exp.timestamp(),
            jti: format!("{}-{}", self.api_key_sid, now.timestamp()),
            grants,
        };

        let header = TwilioJwtHeader {
            typ: "JWT".to_string(),
            alg: "HS256".to_string(),
            cty: "twilio-fpa;v=1".to_string(),
        };

        encode_jwt_with_custom_header(header, &payload, &self.api_key_secret)
    }
}

fn encode_jwt_with_custom_header(
    header: TwilioJwtHeader,
    claims: &JwtClaims,
    secret: &str,
) -> Result<String, JwtError> {
    let header_json =
        serde_json::to_string(&header).map_err(|e| JwtError::EncodingError(e.to_string()))?;
    let claims_json =
        serde_json::to_string(claims).map_err(|e| JwtError::EncodingError(e.to_string()))?;

    let header_b64 = base64_url_encode(header_json.as_bytes());
    let claims_b64 = base64_url_encode(claims_json.as_bytes());

    let message = format!("{}.{}", header_b64, claims_b64);

    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes())
        .map_err(|e| JwtError::EncodingError(e.to_string()))?;
    mac.update(message.as_bytes());
    let signature = mac.finalize().into_bytes();
    let signature_b64 = base64_url_encode(&signature);

    Ok(format!("{}.{}.{}", header_b64, claims_b64, signature_b64))
}

#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    iss: String,
    exp: i64,
    jti: String,
    sub: String,
    grants: Value,
}

fn base64_url_encode(input: &[u8]) -> String {
    use base64::prelude::*;
    BASE64_URL_SAFE_NO_PAD.encode(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{decode, DecodingKey, Validation};

    fn decode_token(token: &str) -> Value {
        let mut validation = Validation::new(jsonwebtoken::Algorithm::HS256);
        validation.validate_exp = false;
        validation.validate_nbf = false;

        let token_data = decode::<Value>(
            token,
            &DecodingKey::from_secret("secret".as_ref()),
            &validation,
        )
        .unwrap();
        token_data.claims
    }

    #[test]
    fn test_video_grant_serialization() {
        let grant = VideoGrant {
            room: "room123".to_string(),
        };

        let json = serde_json::to_string(&grant).unwrap();
        assert!(json.contains("\"room\":\"room123\""));
    }

    #[test]
    fn test_chat_grant_serialization() {
        let grant = ChatGrant {
            service_sid: "sid123".to_string(),
        };

        let json = serde_json::to_string(&grant).unwrap();
        assert!(json.contains("\"service_sid\":\"sid123\""));
    }

    #[test]
    fn test_build_video_token() {
        let builder = TwilioAccessTokenBuilder::new(
            "AC123".to_string(),
            "SK123".to_string(),
            "secret".to_string(),
        );

        let token = builder.build_video_token("room123", "doctor_456").unwrap();
        assert_eq!(token.split('.').count(), 3);

        let claims = decode_token(&token);
        assert_eq!(claims["iss"], "SK123");
        assert_eq!(claims["sub"], "AC123");
        assert_eq!(claims["grants"]["identity"], "doctor_456");
        assert_eq!(claims["grants"]["video"]["room"], "room123");
    }

    #[test]
    fn test_validate_missing_fields() {
        let builder = TwilioAccessTokenBuilder::new(
            "".to_string(),
            "SK123".to_string(),
            "secret".to_string(),
        );

        assert!(builder.validate().is_err());
    }

    #[test]
    fn test_build_video_chat_token() {
        let builder = TwilioAccessTokenBuilder::new(
            "AC123".to_string(),
            "SK123".to_string(),
            "secret".to_string(),
        );

        let token = builder
            .build_video_chat_token("room123", Some("sid123"), "doctor_456")
            .unwrap();

        assert_eq!(token.split('.').count(), 3);

        let claims = decode_token(&token);
        assert_eq!(claims["iss"], "SK123");
        assert_eq!(claims["sub"], "AC123");
        assert_eq!(claims["grants"]["identity"], "doctor_456");
        assert_eq!(claims["grants"]["video"]["room"], "room123");
        assert_eq!(claims["grants"]["chat"]["service_sid"], "sid123");
    }

    #[test]
    fn test_build_chat_token() {
        let builder = TwilioAccessTokenBuilder::new(
            "AC123".to_string(),
            "SK123".to_string(),
            "secret".to_string(),
        );

        let token = builder.build_chat_token("sid123", "patient_456").unwrap();
        assert_eq!(token.split('.').count(), 3);

        let claims = decode_token(&token);
        assert_eq!(claims["iss"], "SK123");
        assert_eq!(claims["sub"], "AC123");
        assert_eq!(claims["grants"]["identity"], "patient_456");
        assert_eq!(claims["grants"]["chat"]["service_sid"], "sid123");
    }

    #[test]
    fn test_jwt_fields() {
        let builder = TwilioAccessTokenBuilder::new(
            "ACaccount".to_string(),
            "SKkey".to_string(),
            "secret".to_string(),
        );

        let token = builder.build_video_token("room", "user").unwrap();
        let claims = decode_token(&token);

        assert!(claims.get("jti").is_some());
        assert!(claims.get("exp").is_some());
        assert!(claims.get("iss").is_some());
        assert!(claims.get("sub").is_some());
        assert!(claims.get("grants").is_some());

        let header = decode_header(&token);
        assert_eq!(header["typ"], "JWT");
        assert_eq!(header["alg"], "HS256");
        assert_eq!(header["cty"], "twilio-fpa;v=1");
    }

    fn decode_header(token: &str) -> Value {
        let header_part = token.split('.').next().unwrap();
        let decoded = base64_decode_url_safe(header_part);
        let json_str = String::from_utf8(decoded).unwrap();
        serde_json::from_str(&json_str).unwrap()
    }

    fn base64_decode_url_safe(input: &str) -> Vec<u8> {
        let mut input = input.replace('-', "+").replace('_', "/");
        while input.len() % 4 != 0 {
            input.push('=');
        }
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(input)
            .unwrap()
    }
}
