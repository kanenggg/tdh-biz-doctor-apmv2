use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};

use crate::appointment::get_detail::model::GetAppointmentDetailResponse;
use crate::appointment::get_detail::service::GetAppointmentDetailError;
use crate::appointment::state::AppState;
use crate::common::{TraceError, internal_error_msg};

#[utoipa::path(
    get,
    path = "/internal/v1/appointment/{bookingId}",
    tag = "appointment",
    params(
        ("bookingId" = String, Path, description = "Appointment booking id")
    ),
    responses(
        (status = 200, description = "Appointment detail or AppointmentNotFound", body = GetAppointmentDetailResponse),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_appointment_detail(
    State(state): State<AppState>,
    Path(booking_id): Path<String>,
) -> Result<Json<GetAppointmentDetailResponse>, TraceError> {
    tracing::info!(booking_id = %booking_id, "get_appointment_detail start");

    match state
        .get_appointment_detail_service
        .get_appointment_detail(&booking_id)
        .await
    {
        Ok(resp) => Ok(Json(resp)),
        Err(e) => {
            tracing::error!(error = %e, booking_id = %booking_id, "get_appointment_detail failed");
            match e {
                GetAppointmentDetailError::DatabaseError(_) => {
                    Err(internal_error_msg("Failed to fetch appointment detail"))
                }
                GetAppointmentDetailError::UnsupportedPrescreenDataType(_)
                | GetAppointmentDetailError::PrescreenBase64Error(_)
                | GetAppointmentDetailError::PrescreenKmsError(_)
                | GetAppointmentDetailError::PrescreenUtf8Error(_)
                | GetAppointmentDetailError::PrescreenParseError(_) => {
                    Err(internal_error_msg("Failed to decode prescreen data"))
                }
            }
        }
    }
}

const PREFERRED_INTERNAL_GET_DETAIL_ROUTES: &[&str] = &[
    "/internal/v1/appointment/{bookingId}",
    "/internal/v1/appointments/{bookingId}",
];

// Legacy aliases retained during cleanup from the legacy /v2/internal convention.
const LEGACY_INTERNAL_GET_DETAIL_ROUTES: &[&str] = &[
    "/v2/internal/appointment/{bookingId}",
    "/v2/internal/appointments/{bookingId}",
];

pub fn router() -> Router<AppState> {
    PREFERRED_INTERNAL_GET_DETAIL_ROUTES
        .iter()
        .chain(LEGACY_INTERNAL_GET_DETAIL_ROUTES.iter())
        .fold(Router::new(), |router, path| {
            router.route(path, get(get_appointment_detail))
        })
}
