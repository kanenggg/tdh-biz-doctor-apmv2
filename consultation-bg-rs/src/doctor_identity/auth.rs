use std::{collections::HashSet, time::Duration};

use axum::http::HeaderMap;
use serde::Deserialize;

use crate::sys::config::DoctorProjectionSyncConfig;

const TOKENINFO_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct DoctorProjectionSyncAuth {
    client: reqwest::Client,
    allowed_service_account_emails: HashSet<String>,
    tokeninfo_url: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DoctorProjectionSyncAuthError {
    #[error("doctor projection sync auth is not configured")]
    NotConfigured,
    #[error("missing bearer token")]
    MissingBearerToken,
    #[error("invalid bearer token")]
    InvalidBearerToken,
    #[error("token validation failed: {0}")]
    ValidationFailed(String),
    #[error("token service account email is not allowed")]
    Forbidden,
}

#[derive(Debug, Deserialize)]
struct TokenInfoResponse {
    email: Option<String>,
    #[allow(dead_code)]
    expires_in: Option<String>,
    #[allow(dead_code)]
    scope: Option<String>,
}

impl DoctorProjectionSyncAuth {
    pub fn new(config: DoctorProjectionSyncConfig) -> Self {
        Self::new_with_timeout(config, TOKENINFO_REQUEST_TIMEOUT)
    }

    fn new_with_timeout(
        config: DoctorProjectionSyncConfig,
        tokeninfo_request_timeout: Duration,
    ) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(tokeninfo_request_timeout)
                .build()
                .expect("doctor projection sync tokeninfo client should build"),
            allowed_service_account_emails: config
                .allowed_service_account_emails
                .into_iter()
                .map(|email| email.trim().to_ascii_lowercase())
                .filter(|email| !email.is_empty())
                .collect(),
            tokeninfo_url: config.tokeninfo_url,
        }
    }

    pub async fn authorize(
        &self,
        headers: &HeaderMap,
    ) -> Result<String, DoctorProjectionSyncAuthError> {
        if self.allowed_service_account_emails.is_empty() {
            return Err(DoctorProjectionSyncAuthError::NotConfigured);
        }

        let token =
            bearer_token(headers).ok_or(DoctorProjectionSyncAuthError::MissingBearerToken)?;
        let tokeninfo_url = format!(
            "{}?access_token={}",
            self.tokeninfo_url,
            urlencoding::encode(token)
        );
        let info = self
            .client
            .get(tokeninfo_url)
            .send()
            .await
            .map_err(token_validation_transport_error)?;

        if !info.status().is_success() {
            return Err(DoctorProjectionSyncAuthError::InvalidBearerToken);
        }

        let info = info
            .json::<TokenInfoResponse>()
            .await
            .map_err(token_validation_json_error)?;
        let email = info
            .email
            .map(|email| email.to_ascii_lowercase())
            .ok_or(DoctorProjectionSyncAuthError::InvalidBearerToken)?;

        if !self.allowed_service_account_emails.contains(&email) {
            return Err(DoctorProjectionSyncAuthError::Forbidden);
        }

        Ok(email)
    }
}

fn token_validation_transport_error(_: reqwest::Error) -> DoctorProjectionSyncAuthError {
    DoctorProjectionSyncAuthError::ValidationFailed("token validation transport error".to_string())
}

fn token_validation_json_error(_: reqwest::Error) -> DoctorProjectionSyncAuthError {
    DoctorProjectionSyncAuthError::ValidationFailed(
        "token validation response parse error".to_string(),
    )
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    value
        .strip_prefix("Bearer ")
        .or_else(|| value.strip_prefix("bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        sync::oneshot,
    };

    async fn tokeninfo_url_with_response(response: impl Into<String>) -> String {
        let response = response.into();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test tokeninfo listener should bind");
        let addr = listener
            .local_addr()
            .expect("test tokeninfo listener should have local address");

        tokio::spawn(async move {
            let (mut stream, _) = listener
                .accept()
                .await
                .expect("test tokeninfo listener should accept one request");
            let mut buffer = [0_u8; 1024];
            let _ = stream.read(&mut buffer).await;
            stream
                .write_all(response.as_bytes())
                .await
                .expect("test tokeninfo response should be written");
        });

        format!("http://{addr}/tokeninfo")
    }

    async fn tokeninfo_url_recording_request(
        response: impl Into<String>,
    ) -> (String, oneshot::Receiver<String>) {
        let response = response.into();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test tokeninfo listener should bind");
        let addr = listener
            .local_addr()
            .expect("test tokeninfo listener should have local address");
        let (request_tx, request_rx) = oneshot::channel();

        tokio::spawn(async move {
            let (mut stream, _) = listener
                .accept()
                .await
                .expect("test tokeninfo listener should accept one request");
            let mut buffer = [0_u8; 1024];
            let bytes_read = stream
                .read(&mut buffer)
                .await
                .expect("test tokeninfo request should be read");
            let request = String::from_utf8_lossy(&buffer[..bytes_read]).to_string();
            let _ = request_tx.send(request);
            stream
                .write_all(response.as_bytes())
                .await
                .expect("test tokeninfo response should be written");
        });

        (format!("http://{addr}/tokeninfo"), request_rx)
    }

    #[test]
    fn extracts_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer token-1"),
        );

        assert_eq!(bearer_token(&headers), Some("token-1"));
    }

    #[test]
    fn extracts_lowercase_bearer_token_and_trims_whitespace() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("bearer   token-2   "),
        );

        assert_eq!(bearer_token(&headers), Some("token-2"));
    }

    #[test]
    fn rejects_missing_bearer_token() {
        assert_eq!(bearer_token(&HeaderMap::new()), None);
    }

    #[test]
    fn new_normalizes_allowed_service_account_emails() {
        let auth = DoctorProjectionSyncAuth::new(DoctorProjectionSyncConfig {
            allowed_service_account_emails: vec![
                " Sync-Caller@Example.COM ".to_string(),
                "".to_string(),
                "   ".to_string(),
            ],
            tokeninfo_url: "https://tokeninfo.example.test".to_string(),
        });

        assert_eq!(auth.allowed_service_account_emails.len(), 1);
        assert!(
            auth.allowed_service_account_emails
                .contains("sync-caller@example.com")
        );
    }

    #[tokio::test]
    async fn authorize_returns_not_configured_when_allowlist_is_empty() {
        let auth = DoctorProjectionSyncAuth::new(DoctorProjectionSyncConfig {
            allowed_service_account_emails: vec![],
            tokeninfo_url: "http://127.0.0.1:1/tokeninfo".to_string(),
        });

        let result = auth.authorize(&HeaderMap::new()).await;

        assert!(matches!(
            result,
            Err(DoctorProjectionSyncAuthError::NotConfigured)
        ));
    }

    #[tokio::test]
    async fn authorize_rejects_missing_bearer_token_before_tokeninfo_call() {
        let auth = DoctorProjectionSyncAuth::new(DoctorProjectionSyncConfig {
            allowed_service_account_emails: vec!["sync-caller@example.com".to_string()],
            tokeninfo_url: "http://127.0.0.1:1/tokeninfo".to_string(),
        });

        let result = auth.authorize(&HeaderMap::new()).await;

        assert!(matches!(
            result,
            Err(DoctorProjectionSyncAuthError::MissingBearerToken)
        ));
    }

    #[tokio::test]
    async fn authorize_rejects_blank_bearer_token_before_tokeninfo_call() {
        let auth = DoctorProjectionSyncAuth::new(DoctorProjectionSyncConfig {
            allowed_service_account_emails: vec!["sync-caller@example.com".to_string()],
            tokeninfo_url: "http://127.0.0.1:1/tokeninfo".to_string(),
        });
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer    "),
        );

        let result = auth.authorize(&headers).await;

        assert!(matches!(
            result,
            Err(DoctorProjectionSyncAuthError::MissingBearerToken)
        ));
    }

    #[tokio::test]
    async fn authorize_maps_tokeninfo_non_success_to_invalid_bearer_token() {
        let auth = DoctorProjectionSyncAuth::new(DoctorProjectionSyncConfig {
            allowed_service_account_emails: vec!["sync-caller@example.com".to_string()],
            tokeninfo_url: tokeninfo_url_with_response(
                "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n",
            )
            .await,
        });
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer token-1"),
        );

        let result = auth.authorize(&headers).await;

        assert!(matches!(
            result,
            Err(DoctorProjectionSyncAuthError::InvalidBearerToken)
        ));
    }

    #[tokio::test]
    async fn authorize_rejects_tokeninfo_email_not_in_allowlist() {
        let body = r#"{"email":"other@example.com"}"#;
        let auth = DoctorProjectionSyncAuth::new(DoctorProjectionSyncConfig {
            allowed_service_account_emails: vec!["sync-caller@example.com".to_string()],
            tokeninfo_url: tokeninfo_url_with_response(format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            ))
            .await,
        });
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer token-1"),
        );

        let result = auth.authorize(&headers).await;

        assert!(matches!(
            result,
            Err(DoctorProjectionSyncAuthError::Forbidden)
        ));
    }

    #[tokio::test]
    async fn authorize_accepts_allowlisted_tokeninfo_email_case_insensitively() {
        let body = r#"{"email":"Sync-Caller@Example.COM"}"#;
        let auth = DoctorProjectionSyncAuth::new(DoctorProjectionSyncConfig {
            allowed_service_account_emails: vec!["sync-caller@example.com".to_string()],
            tokeninfo_url: tokeninfo_url_with_response(format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            ))
            .await,
        });
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer token-1"),
        );

        let result = auth.authorize(&headers).await;

        assert_eq!(result.unwrap(), "sync-caller@example.com");
    }

    #[tokio::test]
    async fn validation_failed_error_does_not_expose_bearer_token_or_tokeninfo_url() {
        let token = "secret-token-for-log-leak+with/slash?x=1&y=two";
        let encoded_token = urlencoding::encode(token).to_string();
        let tokeninfo_url = "http://127.0.0.1:1/tokeninfo";
        let auth = DoctorProjectionSyncAuth::new(DoctorProjectionSyncConfig {
            allowed_service_account_emails: vec!["sync-caller@example.com".to_string()],
            tokeninfo_url: tokeninfo_url.to_string(),
        });
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}"))
                .expect("test bearer token should be a valid header value"),
        );

        let result = auth.authorize(&headers).await;
        let error = result.expect_err("transport failure should be a validation error");
        let error_text = error.to_string();

        assert!(
            matches!(error, DoctorProjectionSyncAuthError::ValidationFailed(_)),
            "unexpected auth error: {error:?}"
        );
        assert!(
            !error_text.contains(token),
            "validation error exposed raw bearer token: {error_text}"
        );
        assert!(
            !error_text.contains(&encoded_token),
            "validation error exposed encoded bearer token: {error_text}"
        );
        assert!(
            !error_text.contains("access_token"),
            "validation error exposed token query parameter: {error_text}"
        );
        assert!(
            !error_text.contains(tokeninfo_url),
            "validation error exposed tokeninfo URL: {error_text}"
        );
    }

    #[tokio::test]
    async fn tokeninfo_parse_error_does_not_expose_bearer_token_or_tokeninfo_url() {
        let token = "secret-token-for-parse-leak+with/slash?x=1&y=two";
        let encoded_token = urlencoding::encode(token).to_string();
        let tokeninfo_body = "not-json-tokeninfo-response";
        let tokeninfo_url = tokeninfo_url_with_response(format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            tokeninfo_body.len(),
            tokeninfo_body
        ))
        .await;
        let auth = DoctorProjectionSyncAuth::new(DoctorProjectionSyncConfig {
            allowed_service_account_emails: vec!["sync-caller@example.com".to_string()],
            tokeninfo_url: tokeninfo_url.clone(),
        });
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}"))
                .expect("test bearer token should be a valid header value"),
        );

        let result = auth.authorize(&headers).await;
        let error = result.expect_err("parse failure should be a validation error");
        let error_text = error.to_string();

        assert!(
            matches!(error, DoctorProjectionSyncAuthError::ValidationFailed(_)),
            "unexpected auth error: {error:?}"
        );
        assert_eq!(
            error_text,
            "token validation failed: token validation response parse error"
        );
        assert!(
            !error_text.contains(token),
            "validation error exposed raw bearer token: {error_text}"
        );
        assert!(
            !error_text.contains(&encoded_token),
            "validation error exposed encoded bearer token: {error_text}"
        );
        assert!(
            !error_text.contains("access_token"),
            "validation error exposed token query parameter: {error_text}"
        );
        assert!(
            !error_text.contains(&tokeninfo_url),
            "validation error exposed tokeninfo URL: {error_text}"
        );
    }

    async fn tokeninfo_url_that_hangs() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test tokeninfo listener should bind");
        let addr = listener
            .local_addr()
            .expect("test tokeninfo listener should have local address");

        tokio::spawn(async move {
            let (_stream, _) = listener
                .accept()
                .await
                .expect("test tokeninfo listener should accept one request");
            tokio::time::sleep(Duration::from_secs(60)).await;
        });

        format!("http://{addr}/tokeninfo")
    }

    #[tokio::test]
    async fn authorize_times_out_when_tokeninfo_does_not_respond() {
        let auth = DoctorProjectionSyncAuth::new_with_timeout(
            DoctorProjectionSyncConfig {
                allowed_service_account_emails: vec!["sync-caller@example.com".to_string()],
                tokeninfo_url: tokeninfo_url_that_hangs().await,
            },
            Duration::from_millis(20),
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer token-1"),
        );

        let result = auth.authorize(&headers).await;

        assert!(matches!(
            result,
            Err(DoctorProjectionSyncAuthError::ValidationFailed(_))
        ));
    }

    #[tokio::test]
    async fn authorize_urlencodes_bearer_token_in_tokeninfo_request() {
        let body = r#"{"email":"sync-caller@example.com"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        let (tokeninfo_url, request_rx) = tokeninfo_url_recording_request(response).await;
        let auth = DoctorProjectionSyncAuth::new(DoctorProjectionSyncConfig {
            allowed_service_account_emails: vec!["sync-caller@example.com".to_string()],
            tokeninfo_url,
        });
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer token+with/slash?x=1&y=two"),
        );

        let result = auth.authorize(&headers).await;
        let request = request_rx
            .await
            .expect("test tokeninfo request should be recorded");

        assert_eq!(result.unwrap(), "sync-caller@example.com");
        assert!(
            request
                .starts_with("GET /tokeninfo?access_token=token%2Bwith%2Fslash%3Fx%3D1%26y%3Dtwo "),
            "unexpected tokeninfo request: {request}"
        );
    }
}
