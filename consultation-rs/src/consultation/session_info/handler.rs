use crate::common::tdh_protocol::{
    consultation::v2::session_info::GetSessionInfoResult,
    iam::user_identity::{AccountType, UserIdentity},
};
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    routing::get,
};

use super::service::SessionError;
use crate::common::{TraceError, internal_error};
use crate::consultation::state::AppState;

const SESSION_INFO_PATH: &str = "/v2/consultation/session-info/{booking_id}";

#[utoipa::path(
    get,
    path = SESSION_INFO_PATH,
    tag = "consultation",
    params(
        ("booking_id" = String, Path, description = "Booking ID / Appointment ID")
    ),
    responses(
        (status = 200, description = "Session info retrieved successfully", body = GetSessionInfoResult),
        (status = 401, description = "Unauthorized - cannot access this session"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("ApiKeyAuth" = [])
    )
)]
async fn get_session_info(
    Path(booking_id): Path<String>,
    State(state): State<AppState>,
    Extension(user_identity): Extension<UserIdentity>,
) -> Result<Json<GetSessionInfoResult>, TraceError> {
    let patient_identity = (user_identity.account_type == AccountType::Patient).then_some((
        user_identity.tenant_id,
        user_identity.account_id,
        user_identity.user_profile_id,
    ));
    let mut result = match state
        .session_service
        .get_or_create_session(user_identity, &booking_id)
        .await
    {
        Ok(result) => result,
        Err(SessionError::GetSessionInfoResult(r)) => r.0,
        Err(e) => {
            tracing::error!(
                "Failed to get consultation session: booking({}), {}",
                booking_id,
                e
            );
            return Err(internal_error());
        }
    };

    // Only the authorized patient receives an RTDB custom token. Doctors retain
    // the normal session-info response and never receive patient RTDB access.
    if let (
        Some((tenant_id, patient_account_id, patient_profile_id)),
        GetSessionInfoResult::SessionReady(session_ready),
    ) = (patient_identity, &mut result)
    {
        session_ready.rtdb_access = state
            .rtdb_token_issuer
            .issue_for_patient(
                &booking_id,
                tenant_id,
                patient_account_id,
                patient_profile_id,
                session_ready.session_end_time,
                jiff::Timestamp::now().as_second(),
            )
            .await
            .map_err(|e| {
                tracing::error!(booking_id, error = %e, "Failed to issue patient RTDB token");
                internal_error()
            })?;
    }

    Ok(Json(result))
}

pub fn router() -> Router<AppState> {
    Router::new().route(SESSION_INFO_PATH, get(get_session_info))
}
