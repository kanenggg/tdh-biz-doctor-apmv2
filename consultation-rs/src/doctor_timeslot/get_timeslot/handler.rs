use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};

use crate::common::{TraceError, internal_error_msg};
use crate::doctor_timeslot::get_timeslot::model::{
    AvailableDoctorsQuery, AvailableDoctorsResponse, AvailableTimeslotQuery,
    AvailableTimeslotResponse, ScheduledAvailabilityQuery,
};
use crate::doctor_timeslot::get_timeslot::service::GetDoctorTimeslotError;
use crate::doctor_timeslot::state::AppState;

const AVAILABLE_TIMESLOTS_PATH: &str = "/v2/doctor-timeslot/available-timeslots";
const AVAILABLE_DOCTORS_PATH: &str = "/v2/doctor-timeslot/available-doctors";
const SCHEDULED_AVAILABILITY_PATH: &str = "/v2/doctor-timeslot/scheduled-availability";

#[utoipa::path(
    get,
    path = AVAILABLE_TIMESLOTS_PATH,
    tag = "doctor-timeslot",
    params(AvailableTimeslotQuery),
    responses(
        (status = 200, description = "Available doctor timeslots", body = AvailableTimeslotResponse),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "Doctor identity not provisioned/inactive"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_available_timeslots(
    State(state): State<AppState>,
    Query(query): Query<AvailableTimeslotQuery>,
) -> Result<Json<AvailableTimeslotResponse>, TraceError> {
    state
        .get_timeslot_service
        .get_available_timeslots(
            query.doctor_account_id,
            query.doctor_profile_id,
            &query.from_datetime,
            &query.to_datetime,
            query.consultation_channel,
        )
        .await
        .map(Json)
        .map_err(map_error)
}

#[utoipa::path(
    get,
    path = AVAILABLE_DOCTORS_PATH,
    tag = "doctor-timeslot",
    params(AvailableDoctorsQuery),
    responses(
        (status = 200, description = "Doctors with at least one scheduled availability timeslot on the requested local date", body = AvailableDoctorsResponse),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_available_doctors(
    State(state): State<AppState>,
    Query(query): Query<AvailableDoctorsQuery>,
) -> Result<Json<AvailableDoctorsResponse>, TraceError> {
    state
        .get_timeslot_service
        .search_available_doctors(&query.date, &query.timezone, query.consultation_channel)
        .await
        .map(Json)
        .map_err(map_error)
}

#[utoipa::path(
    get,
    path = SCHEDULED_AVAILABILITY_PATH,
    tag = "doctor-timeslot",
    params(ScheduledAvailabilityQuery),
    responses(
        (status = 200, description = "Scheduled availability timeslots for a stable doctor ID on the requested local date", body = AvailableTimeslotResponse),
        (status = 400, description = "Invalid request"),
        (status = 409, description = "Doctor identity not provisioned/inactive"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_scheduled_availability(
    State(state): State<AppState>,
    Query(query): Query<ScheduledAvailabilityQuery>,
) -> Result<Json<AvailableTimeslotResponse>, TraceError> {
    state
        .get_timeslot_service
        .get_scheduled_availability_by_doctor_date(
            query.doctor_id,
            &query.date,
            &query.timezone,
            query.consultation_channel,
        )
        .await
        .map(Json)
        .map_err(map_error)
}

fn map_error(error: GetDoctorTimeslotError) -> TraceError {
    match error {
        GetDoctorTimeslotError::InvalidRequest(message) => TraceError::BadRequest(message),
        GetDoctorTimeslotError::DoctorIdentityNotProvisioned
        | GetDoctorTimeslotError::DoctorInactive => TraceError::Conflict(error.to_string()),
        GetDoctorTimeslotError::Repository(error) => {
            tracing::error!(%error, "get available timeslots failed");
            internal_error_msg("Failed to get available timeslots")
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(AVAILABLE_TIMESLOTS_PATH, get(get_available_timeslots))
        .route(AVAILABLE_DOCTORS_PATH, get(get_available_doctors))
        .route(SCHEDULED_AVAILABILITY_PATH, get(get_scheduled_availability))
}

#[cfg(test)]
mod tests {
    use axum::{http::StatusCode, response::IntoResponse};

    use super::*;

    fn status_for(error: GetDoctorTimeslotError) -> StatusCode {
        map_error(error).into_response().status()
    }

    #[test]
    fn maps_invalid_request_to_http_400() {
        assert_eq!(
            status_for(GetDoctorTimeslotError::InvalidRequest(
                "bad range".to_string(),
            )),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn maps_doctor_identity_not_provisioned_to_http_409() {
        assert_eq!(
            status_for(GetDoctorTimeslotError::DoctorIdentityNotProvisioned),
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn maps_doctor_inactive_to_http_409() {
        assert_eq!(
            status_for(GetDoctorTimeslotError::DoctorInactive),
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn maps_repository_error_to_http_500() {
        assert_eq!(
            status_for(GetDoctorTimeslotError::Repository(anyhow::anyhow!(
                "db down"
            ))),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
