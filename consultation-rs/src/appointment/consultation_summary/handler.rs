use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};

use crate::appointment::consultation_summary::model::ConsultationSummaryResponse;
use crate::appointment::consultation_summary::service::ConsultationSummaryError;
use crate::appointment::state::AppState;
use crate::common::{TraceError, internal_error_msg};

#[utoipa::path(
    get,
    path = "/internal/v1/appointment/{bookingId}/consultation-summary",
    tag = "appointment",
    params(
        ("bookingId" = String, Path, description = "Appointment booking id")
    ),
    responses(
        (status = 200, description = "Consultation summary, NotFound, or NotFulfilled", body = ConsultationSummaryResponse),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_consultation_summary(
    State(state): State<AppState>,
    Path(booking_id): Path<String>,
) -> Result<Json<ConsultationSummaryResponse>, TraceError> {
    tracing::info!(booking_id = %booking_id, "get_consultation_summary start");

    match state
        .consultation_summary_service
        .get_consultation_summary(&booking_id)
        .await
    {
        Ok(resp) => Ok(Json(resp)),
        Err(e) => {
            tracing::error!(error = %e, booking_id = %booking_id, "get_consultation_summary failed");
            match e {
                ConsultationSummaryError::DatabaseError(_) => {
                    Err(internal_error_msg("Failed to fetch consultation summary"))
                }
                ConsultationSummaryError::UnsupportedDataType(_)
                | ConsultationSummaryError::Base64Error(_)
                | ConsultationSummaryError::KmsError(_)
                | ConsultationSummaryError::Utf8Error(_)
                | ConsultationSummaryError::ParseError(_) => {
                    Err(internal_error_msg("Failed to decode summary note"))
                }
            }
        }
    }
}

const PREFERRED_INTERNAL_CONSULTATION_SUMMARY_ROUTE: &str =
    "/internal/v1/appointment/{bookingId}/consultation-summary";

// Legacy alias retained during cleanup from the legacy /v2/internal convention.
const LEGACY_INTERNAL_CONSULTATION_SUMMARY_ROUTE: &str =
    "/v2/internal/appointment/{bookingId}/consultation-summary";

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            PREFERRED_INTERNAL_CONSULTATION_SUMMARY_ROUTE,
            get(get_consultation_summary),
        )
        .route(
            LEGACY_INTERNAL_CONSULTATION_SUMMARY_ROUTE,
            get(get_consultation_summary),
        )
}
