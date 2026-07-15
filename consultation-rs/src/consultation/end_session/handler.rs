use crate::common::tdh_protocol::{
    consultation::{TerminationCode, v2::end_session::EndSessionResult},
    iam::user_identity::{AccountType, UserIdentity},
};
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    routing::post,
};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::common::{TraceError, internal_error};
use crate::consultation::state::AppState;

const END_SESSION_PATH: &str = "/v2/consultation/end-session/{booking_id}";

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EndSessionRequest {
    #[serde(default)]
    pub termination_code: EndSessionTerminateCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason_note: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, ToSchema)]
#[serde(rename_all = "PascalCase")]
pub enum EndSessionTerminateCode {
    #[default]
    SuccessfulSession,
    PatientAbsent,
    DoctorAbsent,
    BothPartiesAbsent,
    TechnicalError,
    PatientVerificationMismatch,
}

impl EndSessionRequest {
    fn into_termination_code(self) -> TerminationCode {
        match self.termination_code {
            EndSessionTerminateCode::SuccessfulSession => TerminationCode::SuccessfulSession {
                patient_joined_at: 0,
                doctor_joined_at: 0,
            },
            EndSessionTerminateCode::PatientAbsent => TerminationCode::PatientAbsent {
                doctor_joined_at: 0,
            },
            EndSessionTerminateCode::DoctorAbsent => TerminationCode::DoctorAbsent {
                patient_joined_at: 0,
            },
            EndSessionTerminateCode::BothPartiesAbsent => TerminationCode::BothPartiesAbsent,
            EndSessionTerminateCode::TechnicalError => TerminationCode::TechnicalError {
                error_message: self
                    .reason_note
                    .unwrap_or_else(|| "doctor ended session".to_string()),
            },
            EndSessionTerminateCode::PatientVerificationMismatch => {
                TerminationCode::PatientVerificationMismatch
            }
        }
    }
}

fn default_termination_code() -> TerminationCode {
    TerminationCode::SuccessfulSession {
        patient_joined_at: 0,
        doctor_joined_at: 0,
    }
}

#[utoipa::path(
    post,
    path = END_SESSION_PATH,
    tag = "consultation",
    params(
        ("booking_id" = String, Path, description = "Booking ID / Appointment ID to end session for")
    ),
    request_body = EndSessionRequest,
    responses(
        (status = 200, description = "Session ended successfully", body = EndSessionResult),
        (status = 401, description = "Unauthorized - user is not a doctor", body = EndSessionResult),
        (status = 404, description = "Session not found", body = EndSessionResult),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("ApiKeyAuth" = [])
    )
)]
pub async fn end_session(
    Extension(user_identity): Extension<UserIdentity>,
    Path(booking_id): Path<String>,
    State(state): State<AppState>,
    request: Option<Json<EndSessionRequest>>,
) -> Result<Json<EndSessionResult>, TraceError> {
    if user_identity.account_type != AccountType::Doctor {
        return Ok(Json(EndSessionResult::Unauthorized));
    }

    let termination_code = request
        .map(|Json(request)| request.into_termination_code())
        .unwrap_or_else(default_termination_code);

    let result = state
        .end_session_service
        .doctor_end_session(
            &booking_id,
            user_identity.user_profile_id as i64,
            termination_code,
        )
        .await;

    match result {
        Ok(rows_affected) => {
            if rows_affected > 0 {
                Ok(Json(EndSessionResult::Success))
            } else {
                Ok(Json(EndSessionResult::SessionNotFound))
            }
        }
        Err(e) => {
            tracing::error!("Failed to end session for booking({}): {}", booking_id, e);
            Err(internal_error())
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new().route(END_SESSION_PATH, post(end_session))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn termination_code_from_optional_request(
        request: Option<EndSessionRequest>,
    ) -> TerminationCode {
        request
            .map(EndSessionRequest::into_termination_code)
            .unwrap_or_else(default_termination_code)
    }

    #[test]
    fn no_request_body_defaults_to_successful_session_termination_code() {
        let termination_code = termination_code_from_optional_request(None);

        let TerminationCode::SuccessfulSession {
            patient_joined_at,
            doctor_joined_at,
        } = termination_code
        else {
            panic!("expected default successful session termination code");
        };

        assert_eq!(patient_joined_at, 0);
        assert_eq!(doctor_joined_at, 0);
    }

    #[test]
    fn patient_verification_mismatch_request_maps_to_domain_termination_code() {
        let request: EndSessionRequest = serde_json::from_value(serde_json::json!({
            "terminationCode": "PatientVerificationMismatch"
        }))
        .expect("request body should deserialize");

        let termination_code = request.into_termination_code();

        assert!(matches!(
            termination_code,
            TerminationCode::PatientVerificationMismatch
        ));
    }

    #[test]
    fn technical_error_request_uses_reason_note_as_error_message() {
        let request: EndSessionRequest = serde_json::from_value(serde_json::json!({
            "terminationCode": "TechnicalError",
            "reasonNote": "camera failed"
        }))
        .expect("request body should deserialize");

        let termination_code = request.into_termination_code();

        let TerminationCode::TechnicalError { error_message } = termination_code else {
            panic!("expected technical error termination code");
        };

        assert_eq!(error_message, "camera failed");
    }
}
