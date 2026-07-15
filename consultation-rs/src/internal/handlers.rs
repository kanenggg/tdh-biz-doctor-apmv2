use crate::common::tdh_protocol::internal::{
    CreateAppointmentRequest, CreateAppointmentResult, CreateConfirmedInstantAppointmentRequest,
};
use axum::{Json, extract::State, http::StatusCode};

use crate::internal::service::{CreateAppointmentService, CreateConfirmedAppointment};

#[derive(Clone)]
pub struct AppState {
    pub create_confirmed_appointment_service: CreateConfirmedAppointment,
    pub create_appointment_service: CreateAppointmentService,
}

#[utoipa::path(
    post,
    path = "/v2/internal/create-confirmed-appointment",
    tag = "internal",
    request_body = CreateConfirmedInstantAppointmentRequest,
    responses(
        (status = 200, description = "Confirmed appointment created successfully", body = i64),
        (status = 500, description = "Internal server error - database operation failed")
    )
)]
pub async fn create_confirmed_appointment(
    State(state): State<AppState>,
    Json(request): Json<CreateConfirmedInstantAppointmentRequest>,
) -> Result<Json<i64>, StatusCode> {
    tracing::info!(
        "Create confirmed appointment request: patient_id={}, doctor_id={}, biz_unit_id={}, booking_type={:?}",
        request.patient_id.account_id,
        request.doctor_id.account_id,
        request.biz_unit_id,
        request.booking_type
    );

    match state
        .create_confirmed_appointment_service
        .create_confirmed_appointment(request)
        .await
    {
        Ok(booking_id) => {
            tracing::info!("Confirmed appointment created: booking_id={}", booking_id);
            Ok(Json(booking_id))
        }
        Err(e) => {
            tracing::error!("Failed to create confirmed appointment: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[utoipa::path(
    post,
    path = "/v2/internal/create-appointment",
    tag = "internal",
    request_body = CreateAppointmentRequest,
    responses(
        (status = 200, description = "Appointment created successfully", body = CreateAppointmentResult),
        (status = 500, description = "Internal server error - database operation failed")
    )
)]
pub async fn create_appointment(
    State(state): State<AppState>,
    Json(request): Json<CreateAppointmentRequest>,
) -> Result<Json<CreateAppointmentResult>, StatusCode> {
    tracing::info!(
        "Create appointment request: patient_account_id={}, doctor_account_id={}, biz_unit_id={}, status={:?}, booking_type={:?}",
        request.patient_id.account_id,
        request.doctor_id.account_id,
        request.biz_unit_id,
        request.appointment_status,
        request.booking_type
    );

    if let Err(error) = request.validate_star_gate_creation() {
        tracing::warn!(error = %error, "Invalid Star Gate appointment creation request");
        return Err(StatusCode::BAD_REQUEST);
    }

    match state
        .create_appointment_service
        .create_appointment(request)
        .await
    {
        Ok(result) => {
            tracing::info!(
                "Appointment created: booking_id={}, appointment_id={}",
                result.booking_id,
                result.appointment_id
            );
            Ok(Json(result))
        }
        Err(e) => {
            tracing::error!("Failed to create appointment: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
