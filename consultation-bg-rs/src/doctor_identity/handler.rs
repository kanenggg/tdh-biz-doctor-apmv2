use std::sync::Arc;

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use tracing::Instrument;

use crate::common::tdh_protocol::gcp::pubsub_push_message::PubSubPushMessage;

use super::{
    auth::{DoctorProjectionSyncAuth, DoctorProjectionSyncAuthError},
    model::DoctorProfileEvent,
    service::{DoctorIdentityError, DoctorIdentityService},
};

#[derive(Clone)]
pub(crate) struct AppState {
    pub service: Arc<DoctorIdentityService>,
    pub sync_auth: Arc<DoctorProjectionSyncAuth>,
}

pub(crate) fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/internal/v1/pubsub/doctor-profile",
            post(handle_pubsub_push),
        )
        .route("/pubsub/doctor-profile", post(handle_pubsub_push))
        .route(
            "/internal/v1/doctor-projection/sync",
            post(handle_direct_sync),
        )
        .route("/internal/doctor-projection/sync", post(handle_direct_sync))
        .with_state(state)
}

pub(crate) async fn handle_pubsub_push(
    State(state): State<Arc<AppState>>,
    Json(push_msg): Json<PubSubPushMessage>,
) -> StatusCode {
    let message_id = push_msg.message.message_id.clone();
    let traceparent = push_msg
        .message
        .attributes
        .as_ref()
        .and_then(|attrs| attrs.get("googclient_traceparent"))
        .cloned();

    let span = tracing::info_span!(
        "pubsub_doctor_profile",
        message_id = %message_id,
        traceparent = traceparent.as_deref().unwrap_or(""),
    );

    async move {
        let event: DoctorProfileEvent = match push_msg.read_data() {
            Ok(event) => event,
            Err(e) => {
                tracing::error!(error = %e, "failed to decode DoctorProfileEvent Pub/Sub data");
                return StatusCode::INTERNAL_SERVER_ERROR;
            }
        };

        tracing::info!(
            event_type = event.event_type(),
            "processing doctor profile event"
        );

        match state.service.handle_event(event).await {
            Ok(()) => StatusCode::OK,
            Err(DoctorIdentityError::InvalidEvent(e)) => {
                tracing::warn!(error = %e, "invalid DoctorProfileEvent; returning retryable failure for Pub/Sub DLQ policy");
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Err(DoctorIdentityError::MissingApprovedConfig) => {
                tracing::error!("approved doctor profile event lacked committed configuration");
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Err(DoctorIdentityError::ContractConflict) => {
                tracing::error!("doctor profile projection version conflict");
                StatusCode::INTERNAL_SERVER_ERROR
            }
            Err(DoctorIdentityError::Database(e)) => {
                tracing::error!(error = %e, "database error while projecting doctor identity");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
    .instrument(span)
    .await
}

pub(crate) async fn handle_direct_sync(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(event): Json<DoctorProfileEvent>,
) -> StatusCode {
    let caller_email = match state.sync_auth.authorize(&headers).await {
        Ok(email) => email,
        Err(DoctorProjectionSyncAuthError::NotConfigured) => {
            tracing::warn!(
                "doctor projection sync endpoint is disabled because allowlist is empty"
            );
            return StatusCode::SERVICE_UNAVAILABLE;
        }
        Err(DoctorProjectionSyncAuthError::MissingBearerToken)
        | Err(DoctorProjectionSyncAuthError::InvalidBearerToken) => {
            tracing::warn!("doctor projection sync rejected invalid bearer token");
            return StatusCode::UNAUTHORIZED;
        }
        Err(DoctorProjectionSyncAuthError::Forbidden) => {
            tracing::warn!("doctor projection sync rejected forbidden caller");
            return StatusCode::FORBIDDEN;
        }
        Err(DoctorProjectionSyncAuthError::ValidationFailed(e)) => {
            tracing::error!(error = %e, "doctor projection sync token validation failed");
            return StatusCode::INTERNAL_SERVER_ERROR;
        }
    };

    tracing::info!(
        caller_email = %caller_email,
        event_type = event.event_type(),
        "processing direct doctor projection sync"
    );

    match state.service.handle_event(event).await {
        Ok(()) => StatusCode::OK,
        Err(DoctorIdentityError::InvalidEvent(e)) => {
            tracing::warn!(error = %e, "direct doctor projection sync rejected invalid event");
            StatusCode::BAD_REQUEST
        }
        Err(DoctorIdentityError::MissingApprovedConfig) => StatusCode::BAD_REQUEST,
        Err(DoctorIdentityError::ContractConflict) => StatusCode::CONFLICT,
        Err(DoctorIdentityError::Database(e)) => {
            tracing::error!(error = %e, "database error while syncing doctor projection");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use base64::{Engine, prelude::BASE64_STANDARD};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use crate::common::tdh_protocol::gcp::pubsub_push_message::PubSubMessage;
    use crate::doctor_identity::repo::{
        DoctorDeactivationProjection, DoctorIdentityRepo, DoctorProfileProjection,
    };
    use crate::sys::config::DoctorProjectionSyncConfig;

    #[derive(Default)]
    struct RecordingDoctorIdentityRepo {
        calls: AtomicUsize,
        approved_calls: AtomicUsize,
        deactivated_calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl DoctorIdentityRepo for RecordingDoctorIdentityRepo {
        async fn apply_projection(
            &self,
            _projection: DoctorProfileProjection,
        ) -> Result<(), anyhow::Error> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.approved_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn deactivate(
            &self,
            _projection: DoctorDeactivationProjection,
        ) -> Result<(), anyhow::Error> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.deactivated_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[derive(Default)]
    struct FailingDoctorIdentityRepo {
        calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl DoctorIdentityRepo for FailingDoctorIdentityRepo {
        async fn apply_projection(
            &self,
            _projection: DoctorProfileProjection,
        ) -> Result<(), anyhow::Error> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Err(anyhow::anyhow!("doctor identity repo unavailable"))
        }
        async fn deactivate(
            &self,
            _projection: DoctorDeactivationProjection,
        ) -> Result<(), anyhow::Error> {
            Ok(())
        }
    }

    fn doctor_profile_event() -> DoctorProfileEvent {
        serde_json::from_str(include_str!(
            "../../../fixtures/doctor_profile_approved_origin_main.json"
        ))
        .expect("golden DoctorApp fixture must deserialize")
    }

    fn deactivated_doctor_profile_event() -> DoctorProfileEvent {
        serde_json::from_value(serde_json::json!({
            "__type": "DoctorProfileDeactivated", "eventId": "evt-deactivated",
            "doctorId": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
            "doctorAccountId": 2443, "doctorProfileId": 8891,
            "reason": "license expired", "deactivatedAt": 1718668800, "occurredAt": 1718668801
        }))
        .expect("committed deactivation fixture must deserialize")
    }

    fn push_message(data: impl Into<String>) -> PubSubPushMessage {
        PubSubPushMessage {
            message: PubSubMessage {
                data: data.into(),
                message_id: "test-message-id".to_string(),
                attributes: None,
            },
            subscription: "test-subscription".to_string(),
        }
    }

    fn push_message_json(data: impl Into<String>) -> serde_json::Value {
        serde_json::json!({
            "message": {
                "data": data.into(),
                "message_id": "test-message-id",
                "attributes": null
            },
            "subscription": "test-subscription"
        })
    }

    fn doctor_profile_event_json() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../../fixtures/doctor_profile_approved_origin_main.json"
        ))
        .expect("golden DoctorApp fixture must be JSON")
    }

    fn encoded_doctor_profile_event() -> String {
        BASE64_STANDARD.encode(serde_json::to_vec(&doctor_profile_event_json()).unwrap())
    }

    fn encoded_deactivated_doctor_profile_event() -> String {
        let event = serde_json::json!({
            "__type": "DoctorProfileDeactivated", "eventId": "evt-deactivated",
            "doctorId": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
            "doctorAccountId": 2443, "doctorProfileId": 8891,
            "reason": "license expired", "deactivatedAt": 1718668800, "occurredAt": 1718668801
        });
        BASE64_STANDARD.encode(serde_json::to_vec(&event).unwrap())
    }

    fn encoded_invalid_v2_doctor_profile_event() -> String {
        let mut event = doctor_profile_event_json();
        event["consultationConfig"] = serde_json::json!({
            "channels": ["video"], "languages": ["th"], "durationMinutes": 30,
            "feeAmount": "200.00", "currency": "THB"
        });
        BASE64_STANDARD.encode(serde_json::to_vec(&event).unwrap())
    }

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

    async fn tokeninfo_url_with_email(email: &'static str) -> String {
        let body = format!(r#"{{"email":"{email}"}}"#);
        tokeninfo_url_with_response(format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        ))
        .await
    }

    fn test_state(
        repo: Arc<dyn DoctorIdentityRepo>,
        allowed_service_account_emails: Vec<String>,
    ) -> Arc<AppState> {
        test_state_with_tokeninfo_url(
            repo,
            allowed_service_account_emails,
            "http://127.0.0.1:1/tokeninfo".to_string(),
        )
    }

    fn test_state_with_tokeninfo_url(
        repo: Arc<dyn DoctorIdentityRepo>,
        allowed_service_account_emails: Vec<String>,
        tokeninfo_url: String,
    ) -> Arc<AppState> {
        Arc::new(AppState {
            service: Arc::new(DoctorIdentityService::new(repo, vec![2])),
            sync_auth: Arc::new(DoctorProjectionSyncAuth::new(DoctorProjectionSyncConfig {
                allowed_service_account_emails,
                tokeninfo_url,
            })),
        })
    }

    #[tokio::test]
    async fn pubsub_invalid_data_returns_500_without_repo_call() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let state = test_state(repo.clone(), vec![]);

        let status = handle_pubsub_push(State(state), Json(push_message("not-valid-base64"))).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn pubsub_invalid_v2_contract_returns_retryable_500_without_repo_call() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let state = test_state(repo.clone(), vec![]);
        let status = handle_pubsub_push(
            State(state),
            Json(push_message(encoded_invalid_v2_doctor_profile_event())),
        )
        .await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn pubsub_approved_event_returns_ok_and_calls_repo() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let state = test_state(repo.clone(), vec![]);

        let status = handle_pubsub_push(
            State(state),
            Json(push_message(encoded_doctor_profile_event())),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 1);
        assert_eq!(repo.approved_calls.load(Ordering::SeqCst), 1);
        assert_eq!(repo.deactivated_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn pubsub_deactivated_event_returns_ok_and_projects_inactive_state() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let state = test_state(repo.clone(), vec![]);

        let status = handle_pubsub_push(
            State(state),
            Json(push_message(encoded_deactivated_doctor_profile_event())),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 1);
        assert_eq!(repo.approved_calls.load(Ordering::SeqCst), 0);
        assert_eq!(repo.deactivated_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn direct_sync_not_configured_returns_503_without_repo_call() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let state = test_state(repo.clone(), vec![]);

        let status =
            handle_direct_sync(State(state), HeaderMap::new(), Json(doctor_profile_event())).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn direct_sync_missing_bearer_returns_401_without_repo_call() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let state = test_state(repo.clone(), vec!["sync-caller@example.com".to_string()]);

        let status =
            handle_direct_sync(State(state), HeaderMap::new(), Json(doctor_profile_event())).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn direct_sync_malformed_authorization_returns_401_without_repo_call() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let state = test_state(repo.clone(), vec!["sync-caller@example.com".to_string()]);
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Basic token-1"),
        );

        let status = handle_direct_sync(State(state), headers, Json(doctor_profile_event())).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn direct_sync_blank_bearer_returns_401_without_repo_call() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let state = test_state(repo.clone(), vec!["sync-caller@example.com".to_string()]);
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer    "),
        );

        let status = handle_direct_sync(State(state), headers, Json(doctor_profile_event())).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn direct_sync_authorized_request_returns_ok_and_calls_repo() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let state = test_state_with_tokeninfo_url(
            repo.clone(),
            vec!["sync-caller@example.com".to_string()],
            tokeninfo_url_with_email("sync-caller@example.com").await,
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer token-1"),
        );

        let status = handle_direct_sync(State(state), headers, Json(doctor_profile_event())).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 1);
        assert_eq!(repo.approved_calls.load(Ordering::SeqCst), 1);
        assert_eq!(repo.deactivated_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn direct_sync_authorized_deactivated_request_projects_inactive_state() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let state = test_state_with_tokeninfo_url(
            repo.clone(),
            vec!["sync-caller@example.com".to_string()],
            tokeninfo_url_with_email("sync-caller@example.com").await,
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer token-1"),
        );

        let status = handle_direct_sync(
            State(state),
            headers,
            Json(deactivated_doctor_profile_event()),
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 1);
        assert_eq!(repo.approved_calls.load(Ordering::SeqCst), 0);
        assert_eq!(repo.deactivated_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn direct_sync_forbidden_caller_returns_403_without_repo_call() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let state = test_state_with_tokeninfo_url(
            repo.clone(),
            vec!["sync-caller@example.com".to_string()],
            tokeninfo_url_with_email("other@example.com").await,
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer token-1"),
        );

        let status = handle_direct_sync(State(state), headers, Json(doctor_profile_event())).await;

        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn direct_sync_invalid_bearer_returns_401_without_repo_call() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let state = test_state_with_tokeninfo_url(
            repo.clone(),
            vec!["sync-caller@example.com".to_string()],
            tokeninfo_url_with_response("HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n")
                .await,
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer token-1"),
        );

        let status = handle_direct_sync(State(state), headers, Json(doctor_profile_event())).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn direct_sync_tokeninfo_parse_failure_returns_500_without_repo_call() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let state = test_state_with_tokeninfo_url(
            repo.clone(),
            vec!["sync-caller@example.com".to_string()],
            tokeninfo_url_with_response(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 8\r\n\r\nnot-json",
            )
            .await,
        );
        let token = "secret-token-for-handler-parse-leak+with/slash?x=1&y=two";
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_str(&format!("Bearer {token}"))
                .expect("test bearer token should be a valid header value"),
        );

        let status = handle_direct_sync(State(state), headers, Json(doctor_profile_event())).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn direct_sync_database_error_returns_500_after_authorized_repo_call() {
        let repo = Arc::new(FailingDoctorIdentityRepo::default());
        let state = test_state_with_tokeninfo_url(
            repo.clone(),
            vec!["sync-caller@example.com".to_string()],
            tokeninfo_url_with_email("sync-caller@example.com").await,
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer token-1"),
        );

        let status = handle_direct_sync(State(state), headers, Json(doctor_profile_event())).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(repo.calls.load(Ordering::SeqCst), 1);
    }

    async fn spawn_test_server(state: Arc<AppState>) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test server should bind");
        let addr = listener.local_addr().expect("test server should have addr");
        tokio::spawn(async move {
            axum::serve(listener, routes(state))
                .await
                .expect("test server should serve routes");
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn pubsub_doctor_profile_route_aliases_accept_preferred_and_legacy_paths() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let base_uri = spawn_test_server(test_state(repo.clone(), vec![])).await;
        let client = reqwest::Client::new();

        for path in [
            "/internal/v1/pubsub/doctor-profile",
            "/pubsub/doctor-profile",
        ] {
            let response = client
                .post(format!("{base_uri}{path}"))
                .json(&push_message_json(encoded_doctor_profile_event()))
                .send()
                .await
                .expect("route request should complete");
            assert_eq!(response.status(), StatusCode::OK, "path {path}");
        }

        assert_eq!(repo.calls.load(Ordering::SeqCst), 2);
        assert_eq!(repo.approved_calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn direct_sync_route_aliases_accept_preferred_and_legacy_paths() {
        let repo = Arc::new(RecordingDoctorIdentityRepo::default());
        let base_uri = spawn_test_server(test_state(repo.clone(), vec![])).await;
        let client = reqwest::Client::new();

        for path in [
            "/internal/v1/doctor-projection/sync",
            "/internal/doctor-projection/sync",
        ] {
            let response = client
                .post(format!("{base_uri}{path}"))
                .json(&doctor_profile_event_json())
                .send()
                .await
                .expect("route request should complete");
            assert_eq!(
                response.status(),
                StatusCode::SERVICE_UNAVAILABLE,
                "path {path}"
            );
        }

        assert_eq!(repo.calls.load(Ordering::SeqCst), 0);
    }
}
