use crate::protocol::SummarizationResult;
use crate::protocol::summary_note::SummarizationRequest;
use axum::{Json, Router, extract::State, routing::post};

use crate::common::{TraceError, internal_error};
use tracing::error;

use super::state::AppState;

const ADD_SUMMARY_NOTE_PATH: &str = "/v2/internal/submit-summary-note";

#[utoipa::path(
    post,
    path = ADD_SUMMARY_NOTE_PATH,
    tag = "summarization",
    request_body = SummarizationRequest,
    responses(
        (status = 200, description = "Summary note created or already exists", body = SummarizationResult),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("ApiKeyAuth" = [])
    )
)]
async fn add_summary_note(
    State(state): State<AppState>,
    Json(summary_note): Json<SummarizationRequest>,
) -> Result<Json<SummarizationResult>, TraceError> {
    state
        .summarization_service
        .add_summary_note(None, summary_note)
        .await
        .map(Json)
        .map_err(|e| {
            error!(
                error = %e,
                "Failed to add summary note"
            );
            internal_error()
        })
}

pub fn router() -> Router<AppState> {
    Router::new().route(ADD_SUMMARY_NOTE_PATH, post(add_summary_note))
}
