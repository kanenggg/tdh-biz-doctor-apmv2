use crate::common::tdh_protocol::{
    consultation::v2::patient_verification::PatientIdVerificationResult,
    iam::user_identity::{AccountType, UserIdentity},
};
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::post,
};

use crate::consultation::{
    patient_verification::service::PatientVerificationError, state::AppState,
};

#[utoipa::path(
    post,
    path = "/v2/consultation/patient-id-verify-match/{booking_id}",
    tag = "consultation",
    params(
        ("booking_id" = String, Path, description = "Booking ID / Appointment ID for patient verification match")
    ),
    responses(
        (status = 200, description = "Patient ID verification match recorded successfully", body = PatientIdVerificationResult),
        (status = 401, description = "Unauthorized - user is not a doctor or session status not permitted", body = PatientIdVerificationResult),
        (status = 404, description = "Session not found", body = PatientIdVerificationResult),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("ApiKeyAuth" = [])
    )
)]
async fn match_handle(
    Path(booking_id): Path<String>,
    State(state): State<AppState>,
    Extension(user_identity): Extension<UserIdentity>,
) -> Result<Json<PatientIdVerificationResult>, StatusCode> {
    if user_identity.account_type != AccountType::Doctor {
        return Ok(Json(PatientIdVerificationResult::SessionStatusNotPermitted));
    }

    map_patient_verification_result(
        state
            .patient_verification_service
            .match_handle(&booking_id, user_identity.user_profile_id as i64)
            .await,
    )
    .map(Json)
}

#[utoipa::path(
    post,
    path = "/v2/consultation/patient-id-verify-miss-match/{booking_id}",
    tag = "consultation",
    params(
        ("booking_id" = String, Path, description = "Booking ID / Appointment ID for patient verification mismatch")
    ),
    responses(
        (status = 200, description = "Patient ID verification mismatch recorded successfully", body = PatientIdVerificationResult),
        (status = 401, description = "Unauthorized - user is not a doctor or session status not permitted", body = PatientIdVerificationResult),
        (status = 404, description = "Session not found", body = PatientIdVerificationResult),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("ApiKeyAuth" = [])
    )
)]
async fn miss_match_handle(
    Path(booking_id): Path<String>,
    State(state): State<AppState>,
    Extension(user_identity): Extension<UserIdentity>,
) -> Result<Json<PatientIdVerificationResult>, StatusCode> {
    if user_identity.account_type != AccountType::Doctor {
        return Ok(Json(PatientIdVerificationResult::SessionStatusNotPermitted));
    }

    map_patient_verification_result(
        state
            .patient_verification_service
            .miss_match_handle(&booking_id, user_identity.user_profile_id as i64)
            .await,
    )
    .map(Json)
}

fn map_patient_verification_result(
    result: Result<u64, PatientVerificationError>,
) -> Result<PatientIdVerificationResult, StatusCode> {
    match result {
        Ok(_) => Ok(PatientIdVerificationResult::Success),
        Err(PatientVerificationError::NotFoundOrUnauthorized) => {
            Ok(PatientIdVerificationResult::SessionNotFound)
        }
        Err(PatientVerificationError::Repository(error)) => {
            tracing::error!(%error, "patient verification failed");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v2/consultation/patient-id-verify-match/{booking_id}",
            post(match_handle),
        )
        .route(
            "/v2/consultation/patient-id-verify-miss-match/{booking_id}",
            post(miss_match_handle),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_success_to_success_result() {
        let result = map_patient_verification_result(Ok(1)).expect("success should map");
        assert!(matches!(result, PatientIdVerificationResult::Success));
    }

    #[test]
    fn maps_not_found_or_unauthorized_to_domain_not_found_result() {
        let result =
            map_patient_verification_result(Err(PatientVerificationError::NotFoundOrUnauthorized))
                .expect("not found should map to domain result");
        assert!(matches!(
            result,
            PatientIdVerificationResult::SessionNotFound
        ));
    }

    #[test]
    fn maps_repository_error_to_internal_server_error() {
        let result = map_patient_verification_result(Err(PatientVerificationError::Repository(
            anyhow::anyhow!("db down"),
        )));
        assert!(matches!(result, Err(StatusCode::INTERNAL_SERVER_ERROR)));
    }
}
