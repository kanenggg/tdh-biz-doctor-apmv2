use crate::common::tdh_protocol::gcp::pubsub_push_message::PubSubPushMessage;
use crate::common::tdh_protocol::payment::payment_message::PaymentMessage;
use axum::{Json, extract::State, http::StatusCode};
use std::sync::Arc;
use tracing::Instrument;

use super::service::{PaymentConfirmError, PaymentConfirmService};
use crate::payment_confirm::repo::PaymentConfirmDomainError;

pub(crate) struct AppState {
    pub service: Arc<PaymentConfirmService>,
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
    let span = tracing::info_span!("pubsub_payment_confirm", message_id = %message_id, traceparent = traceparent.as_deref().unwrap_or(""));

    async move {
        let payment_msg: PaymentMessage = match push_msg.read_data() {
            Ok(message) => message,
            Err(error) => {
                tracing::error!(%error, "Failed to decode PubSub payment message");
                return StatusCode::BAD_REQUEST;
            }
        };
        match state.service.handle_payment_tx_v1(payment_msg).await {
            Ok(()) => StatusCode::OK,
            Err(PaymentConfirmError::Domain(domain)) => {
                tracing::warn!(
                    ?domain,
                    "Payment confirmation rejected by domain validation"
                );
                match domain {
                    PaymentConfirmDomainError::HoldNotFound
                    | PaymentConfirmDomainError::InvalidRequest => StatusCode::BAD_REQUEST,
                    PaymentConfirmDomainError::HoldNotBookable
                    | PaymentConfirmDomainError::ReplayConflict
                    | PaymentConfirmDomainError::QuoteMismatch
                    | PaymentConfirmDomainError::MissingOccupancy => StatusCode::CONFLICT,
                }
            }
            Err(
                PaymentConfirmError::UnexpectedExtendData
                | PaymentConfirmError::MissingExtendData
                | PaymentConfirmError::InvalidToken(_),
            ) => StatusCode::BAD_REQUEST,
            Err(error) => {
                tracing::error!(%error, "Payment confirmation infrastructure failure");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
    .instrument(span)
    .await
}
