use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};

use crate::appointment::list::model::{ListAppointmentsQuery, ListAppointmentsResponse};
use crate::appointment::list::service::ListAppointmentsError;
use crate::appointment::state::AppState;
use crate::common::{TraceError, internal_error_msg};

#[utoipa::path(
    get,
    path = "/internal/v1/appointments",
    tag = "appointment",
    params(
        ("patientAccountId" = i32, Query, description = "Patient account id (required)"),
        ("patientProfileId" = Option<i32>, Query, description = "Patient profile id (optional; narrows to one profile)")
    ),
    responses(
        (status = 200, description = "The patient's fulfilled appointments, newest first", body = ListAppointmentsResponse),
        (status = 400, description = "Missing or invalid query parameters"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_appointments(
    State(state): State<AppState>,
    Query(query): Query<ListAppointmentsQuery>,
) -> Result<Json<ListAppointmentsResponse>, TraceError> {
    tracing::info!(
        patient_account_id = query.patient_account_id,
        patient_profile_id = ?query.patient_profile_id,
        "list_appointments start"
    );

    match state
        .list_appointments_service
        .list_appointments(query.patient_account_id, query.patient_profile_id)
        .await
    {
        Ok(resp) => Ok(Json(resp)),
        Err(e) => {
            tracing::error!(error = %e, "list_appointments failed");
            match e {
                ListAppointmentsError::DatabaseError(_) => {
                    Err(internal_error_msg("Failed to list appointments"))
                }
            }
        }
    }
}

const PREFERRED_INTERNAL_LIST_APPOINTMENTS_ROUTE: &str = "/internal/v1/appointments";

// Legacy alias retained during cleanup from the legacy /v2/internal convention.
const LEGACY_INTERNAL_LIST_APPOINTMENTS_ROUTE: &str = "/v2/internal/appointments";

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            PREFERRED_INTERNAL_LIST_APPOINTMENTS_ROUTE,
            get(list_appointments),
        )
        .route(
            LEGACY_INTERNAL_LIST_APPOINTMENTS_ROUTE,
            get(list_appointments),
        )
}
