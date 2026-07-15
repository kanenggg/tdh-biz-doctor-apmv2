use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use base64::Engine;
use common_rs::twilio::TwilioStatusCallback;
use hmac::{Hmac, Mac};
use sha1::Sha1;

use crate::provider_callback::service::ProviderCallbackError;
use crate::provider_callback::state::AppState;

pub async fn twilio_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    if !is_valid_twilio_signature(
        &state.twilio_auth_token,
        &state.twilio_callback_url,
        &headers,
        &body,
    ) {
        return StatusCode::UNAUTHORIZED;
    }

    let payload = match serde_urlencoded::from_bytes::<TwilioStatusCallback>(&body) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::warn!(%error, "twilio callback form payload parse failed after signature validation");
            return StatusCode::OK;
        }
    };

    match state.service.handle_twilio_callback(payload).await {
        Ok(()) => StatusCode::OK,
        Err(ProviderCallbackError::Unsupported) => StatusCode::OK,
        Err(error) => {
            tracing::error!(%error, "twilio callback failed");
            StatusCode::OK
        }
    }
}

fn is_valid_twilio_signature(
    auth_token: &str,
    callback_url: &str,
    headers: &HeaderMap,
    body: &[u8],
) -> bool {
    if auth_token.is_empty() || callback_url.is_empty() {
        return false;
    }

    let Some(signature) = headers
        .get("X-Twilio-Signature")
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };

    expected_twilio_signature(auth_token, callback_url, body)
        .map(|expected| constant_time_eq(expected.as_bytes(), signature.as_bytes()))
        .unwrap_or(false)
}

fn expected_twilio_signature(
    auth_token: &str,
    callback_url: &str,
    body: &[u8],
) -> Result<String, serde_urlencoded::de::Error> {
    let mut params: Vec<(String, String)> = serde_urlencoded::from_bytes(body)?;
    params.sort_by(|left, right| left.0.cmp(&right.0));

    let mut signed_data = String::from(callback_url);
    for (key, value) in params {
        signed_data.push_str(&key);
        signed_data.push_str(&value);
    }

    let mut mac = Hmac::<Sha1>::new_from_slice(auth_token.as_bytes())
        .expect("HMAC accepts auth tokens of any length");
    mac.update(signed_data.as_bytes());
    Ok(base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes()))
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }

    left.iter()
        .zip(right.iter())
        .fold(0u8, |acc, (left, right)| acc | (left ^ right))
        == 0
}

pub fn router() -> Router<AppState> {
    Router::new().route("/twilio/callback", post(twilio_callback))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::tdh_protocol::common::meeting_provider::MeetingProvider;
    use crate::common::tdh_protocol::consultation::ConsultationEvent;
    use crate::consultation::common::SessionDetails;
    use crate::doctor_timeslot::configuration_events::model::DoctorTimeslotConfigChangedEvent;
    use crate::infra::event::EventPublisher;
    use crate::provider_callback::repo::{
        CallbackParticipantRole, ProviderCallbackRepo, TwilioCallbackSessionContext,
    };
    use crate::provider_callback::service::ProviderCallbackService;
    use std::sync::Arc;

    struct PanickingRepo;

    #[async_trait::async_trait]
    impl ProviderCallbackRepo for PanickingRepo {
        async fn insert_callback_event(
            &self,
            _provider_event_id: &str,
            _appointment_id: Option<&str>,
            _event_type: &str,
            _participant_identity: Option<&str>,
            _payload: serde_json::Value,
        ) -> Result<bool, anyhow::Error> {
            panic!("handler test must not insert callback event")
        }

        async fn mark_participant_disconnected(
            &self,
            _appointment_id: &str,
            _role: CallbackParticipantRole,
            _disconnected_at: i64,
        ) -> Result<bool, anyhow::Error> {
            panic!("handler test must not mark participant disconnected")
        }

        async fn get_session_details(
            &self,
            _appointment_id: &str,
        ) -> Result<Option<SessionDetails>, anyhow::Error> {
            panic!("handler test must not fetch session details")
        }

        async fn get_twilio_callback_context(
            &self,
            _appointment_id: &str,
        ) -> Result<Option<TwilioCallbackSessionContext>, anyhow::Error> {
            panic!("handler test must not fetch callback context")
        }
    }

    struct DuplicateRepo;

    #[async_trait::async_trait]
    impl ProviderCallbackRepo for DuplicateRepo {
        async fn insert_callback_event(
            &self,
            _provider_event_id: &str,
            _appointment_id: Option<&str>,
            _event_type: &str,
            _participant_identity: Option<&str>,
            _payload: serde_json::Value,
        ) -> Result<bool, anyhow::Error> {
            Ok(false)
        }

        async fn mark_participant_disconnected(
            &self,
            _appointment_id: &str,
            _role: CallbackParticipantRole,
            _disconnected_at: i64,
        ) -> Result<bool, anyhow::Error> {
            panic!("duplicate callback handler test must not mark participant disconnected")
        }

        async fn get_session_details(
            &self,
            _appointment_id: &str,
        ) -> Result<Option<SessionDetails>, anyhow::Error> {
            panic!("duplicate callback handler test must not fetch session details")
        }

        async fn get_twilio_callback_context(
            &self,
            _appointment_id: &str,
        ) -> Result<Option<TwilioCallbackSessionContext>, anyhow::Error> {
            Ok(Some(TwilioCallbackSessionContext {
                details: session_details(),
                room_sid: Some("RM123".to_string()),
            }))
        }
    }

    struct ErrorRepo;

    #[async_trait::async_trait]
    impl ProviderCallbackRepo for ErrorRepo {
        async fn insert_callback_event(
            &self,
            _provider_event_id: &str,
            _appointment_id: Option<&str>,
            _event_type: &str,
            _participant_identity: Option<&str>,
            _payload: serde_json::Value,
        ) -> Result<bool, anyhow::Error> {
            Err(anyhow::anyhow!("callback store unavailable"))
        }

        async fn mark_participant_disconnected(
            &self,
            _appointment_id: &str,
            _role: CallbackParticipantRole,
            _disconnected_at: i64,
        ) -> Result<bool, anyhow::Error> {
            panic!("error handler test must not mark participant disconnected")
        }

        async fn get_session_details(
            &self,
            _appointment_id: &str,
        ) -> Result<Option<SessionDetails>, anyhow::Error> {
            panic!("error handler test must not fetch session details")
        }

        async fn get_twilio_callback_context(
            &self,
            _appointment_id: &str,
        ) -> Result<Option<TwilioCallbackSessionContext>, anyhow::Error> {
            Ok(Some(TwilioCallbackSessionContext {
                details: session_details(),
                room_sid: Some("RM123".to_string()),
            }))
        }
    }

    struct PanickingPublisher;

    #[async_trait::async_trait]
    impl EventPublisher for PanickingPublisher {
        async fn publish_consultation_event(
            &self,
            _event: ConsultationEvent,
        ) -> Result<(), anyhow::Error> {
            panic!("handler status mapping tests must not publish consultation events")
        }

        async fn publish_doctor_timeslot_config_changed_event(
            &self,
            _event: DoctorTimeslotConfigChangedEvent,
        ) -> Result<(), anyhow::Error> {
            panic!("handler status mapping tests must not publish doctor timeslot events")
        }
    }

    fn app_state(repo: Arc<dyn ProviderCallbackRepo>) -> AppState {
        AppState {
            service: ProviderCallbackService::new(repo, Arc::new(PanickingPublisher)),
            twilio_auth_token: "auth-token".to_string(),
            twilio_callback_url: "https://doctor.test/twilio/callback".to_string(),
        }
    }

    fn session_details() -> SessionDetails {
        SessionDetails {
            appointment_id: "appointment-1".to_string(),
            booking_id: "booking-1".to_string(),
            patient_account_id: 10,
            patient_profile_id: 20,
            tenant_id: 30,
            doctor_id: 40,
            doctor_profile_id: 50,
            session_provider: MeetingProvider::Twilio,
            session_chat_id: Some("CH123".to_string()),
        }
    }

    fn disconnect_form_body() -> Bytes {
        Bytes::from_static(
            b"StatusCallbackEvent=participant-disconnected&RoomName=mordee_twilio_video_appointment-1&RoomSid=RM123&ParticipantIdentity=patient_10_20&ParticipantStatus=disconnected&Timestamp=2026-07-09T01%3A02%3A03Z&SequenceNumber=1",
        )
    }

    fn unsupported_form_body() -> Bytes {
        Bytes::from_static(
            b"StatusCallbackEvent=room-ended&RoomName=mordee_twilio_video_appointment-1&RoomSid=RM123&ParticipantIdentity=patient_10_20&Timestamp=2026-07-09T01%3A02%3A03Z&SequenceNumber=1",
        )
    }

    fn signed_headers(body: &[u8]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        let signature =
            expected_twilio_signature("auth-token", "https://doctor.test/twilio/callback", body)
                .expect("fixture form body should be parseable");
        headers.insert("X-Twilio-Signature", signature.parse().unwrap());
        headers
    }

    async fn call(
        repo: Arc<dyn ProviderCallbackRepo>,
        body: Bytes,
        headers: HeaderMap,
    ) -> StatusCode {
        twilio_callback(State(app_state(repo)), headers, body).await
    }

    #[tokio::test]
    async fn valid_signed_disconnect_callback_returns_ok() {
        let body = disconnect_form_body();
        let status = call(Arc::new(DuplicateRepo), body.clone(), signed_headers(&body)).await;

        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn missing_signature_returns_unauthorized_before_service() {
        let status = call(
            Arc::new(PanickingRepo),
            disconnect_form_body(),
            HeaderMap::new(),
        )
        .await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn invalid_signature_returns_unauthorized_before_service() {
        let body = disconnect_form_body();
        let mut headers = HeaderMap::new();
        headers.insert("X-Twilio-Signature", "invalid".parse().unwrap());

        let status = call(Arc::new(PanickingRepo), body, headers).await;

        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn unsupported_callback_returns_ok_only_after_valid_signature() {
        let body = unsupported_form_body();
        let status = call(Arc::new(PanickingRepo), body.clone(), signed_headers(&body)).await;

        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn duplicate_disconnect_callback_returns_ok() {
        let body = disconnect_form_body();
        let status = call(Arc::new(DuplicateRepo), body.clone(), signed_headers(&body)).await;

        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn repository_error_still_returns_ok_for_twilio_ack_after_valid_signature() {
        let body = disconnect_form_body();
        let status = call(Arc::new(ErrorRepo), body.clone(), signed_headers(&body)).await;

        assert_eq!(status, StatusCode::OK);
    }
}
