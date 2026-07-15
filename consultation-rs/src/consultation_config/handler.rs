use axum::{
    Json, Router,
    extract::{Extension, State},
    routing::{get, put},
};

use super::model::{
    AvailabilityResponse, DoctorAuthContext, ScheduleAvailableConfig, SuccessResponse,
    UpdateAvailabilityRequest, UpdateScheduleConfigResponse,
};
use super::service::{ConsultationConfigError, ConsultationConfigService};
use crate::common::tdh_protocol::iam::user_identity::{AccountType, UserIdentity};
use crate::common::{TraceError, internal_error_msg};

#[derive(Clone)]
pub struct AppState {
    pub service: ConsultationConfigService,
}

#[utoipa::path(
    get,
    path = "/v2/consultation-config/schedule-config",
    tag = "consultation-config",
    responses(
        (status = 200, description = "Doctor schedule configuration", body = ScheduleAvailableConfig),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "Doctor identity not provisioned"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_schedule_config(
    State(state): State<AppState>,
    Extension(user_identity): Extension<UserIdentity>,
) -> Result<Json<ScheduleAvailableConfig>, TraceError> {
    let context = doctor_auth_context(&user_identity)?;
    state
        .service
        .get_schedule_config(context)
        .await
        .map(Json)
        .map_err(map_error)
}

#[utoipa::path(
    put,
    path = "/v2/consultation-config/schedule-config",
    tag = "consultation-config",
    request_body = ScheduleAvailableConfig,
    responses(
        (status = 200, description = "Success or conflict-time-overlap failure", body = UpdateScheduleConfigResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "Doctor identity not provisioned"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_schedule_config(
    State(state): State<AppState>,
    Extension(user_identity): Extension<UserIdentity>,
    Json(req): Json<ScheduleAvailableConfig>,
) -> Result<Json<UpdateScheduleConfigResponse>, TraceError> {
    let context = doctor_auth_context(&user_identity)?;
    state
        .service
        .update_schedule_config(context, req)
        .await
        .map(Json)
        .map_err(map_error)
}

#[utoipa::path(
    get,
    path = "/v2/consultation-config/availability",
    tag = "consultation-config",
    responses(
        (status = 200, description = "Consultation availability", body = AvailabilityResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "Doctor identity not provisioned"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_availability(
    State(state): State<AppState>,
    Extension(user_identity): Extension<UserIdentity>,
) -> Result<Json<AvailabilityResponse>, TraceError> {
    let context = doctor_auth_context(&user_identity)?;
    state
        .service
        .get_availability(context)
        .await
        .map(Json)
        .map_err(map_error)
}

#[utoipa::path(
    put,
    path = "/v2/consultation-config/availability/schedule",
    tag = "consultation-config",
    request_body = UpdateAvailabilityRequest,
    responses(
        (status = 200, description = "Schedule availability saved", body = SuccessResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "Doctor identity not provisioned"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_schedule_availability(
    State(state): State<AppState>,
    Extension(user_identity): Extension<UserIdentity>,
    Json(req): Json<UpdateAvailabilityRequest>,
) -> Result<Json<SuccessResponse>, TraceError> {
    let context = doctor_auth_context(&user_identity)?;
    state
        .service
        .set_schedule_availability(context, req.available)
        .await
        .map(Json)
        .map_err(map_error)
}

#[utoipa::path(
    put,
    path = "/v2/consultation-config/availability/instant",
    tag = "consultation-config",
    request_body = UpdateAvailabilityRequest,
    responses(
        (status = 200, description = "Instant availability saved", body = SuccessResponse),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 409, description = "Doctor identity not provisioned"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_instant_availability(
    State(state): State<AppState>,
    Extension(user_identity): Extension<UserIdentity>,
    Json(req): Json<UpdateAvailabilityRequest>,
) -> Result<Json<SuccessResponse>, TraceError> {
    let context = doctor_auth_context(&user_identity)?;
    state
        .service
        .set_instant_availability(context, req.available)
        .await
        .map(Json)
        .map_err(map_error)
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v2/consultation-config/schedule-config",
            get(get_schedule_config).put(update_schedule_config),
        )
        .route(
            "/v2/consultation-config/availability",
            get(get_availability),
        )
        .route(
            "/v2/consultation-config/availability/schedule",
            put(update_schedule_availability),
        )
        .route(
            "/v2/consultation-config/availability/instant",
            put(update_instant_availability),
        )
}

fn doctor_auth_context(user_identity: &UserIdentity) -> Result<DoctorAuthContext, TraceError> {
    if user_identity.account_type != AccountType::Doctor {
        return Err(TraceError::Forbidden(
            "Only doctors can access consultation config".to_string(),
        ));
    }

    Ok(DoctorAuthContext {
        doctor_account_id: i64::try_from(user_identity.account_id)
            .map_err(|_| TraceError::BadRequest("doctor account id is too large".to_string()))?,
        doctor_profile_id: i64::try_from(user_identity.user_profile_id)
            .map_err(|_| TraceError::BadRequest("doctor profile id is too large".to_string()))?,
    })
}

fn map_error(err: ConsultationConfigError) -> TraceError {
    match err {
        ConsultationConfigError::BadRequest(message) => TraceError::BadRequest(message),
        ConsultationConfigError::DoctorIdentityNotProvisioned => TraceError::Conflict(
            "Doctor identity is not provisioned for this account/profile".to_string(),
        ),
        ConsultationConfigError::Repository(err) => {
            tracing::error!(error = %err, "consultation config repository error");
            internal_error_msg("Failed to persist consultation config")
        }
        ConsultationConfigError::EventPublish(err) => {
            tracing::error!(error = %err, "consultation config event publish error");
            internal_error_msg("Failed to publish consultation config event")
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::{http::StatusCode, response::IntoResponse};

    use super::*;
    use crate::consultation_config::repo::ConsultationConfigRepoError;

    fn user_identity(
        account_type: AccountType,
        account_id: u64,
        user_profile_id: u64,
    ) -> UserIdentity {
        UserIdentity {
            account_id,
            account_type,
            user_profile_id,
            user_main_profile_id: user_profile_id,
            tenant_id: 1,
            oidc_user_id: None,
            legacy_data: None,
        }
    }

    fn status_for(error: ConsultationConfigError) -> StatusCode {
        map_error(error).into_response().status()
    }

    #[test]
    fn doctor_auth_context_allows_doctor_identity() {
        let identity = user_identity(AccountType::Doctor, 123, 456);

        let context = match doctor_auth_context(&identity) {
            Ok(context) => context,
            Err(_) => panic!("doctor identity should be accepted"),
        };

        assert_eq!(context.doctor_account_id, 123);
        assert_eq!(context.doctor_profile_id, 456);
    }

    #[test]
    fn doctor_auth_context_rejects_non_doctor_identity() {
        let identity = user_identity(AccountType::Patient, 123, 456);

        match doctor_auth_context(&identity) {
            Err(TraceError::Forbidden(message)) => {
                assert_eq!(message, "Only doctors can access consultation config");
            }
            Err(_) => panic!("expected forbidden error for non-doctor identity"),
            Ok(_) => panic!("expected non-doctor identity to be rejected"),
        }
    }

    #[test]
    fn doctor_auth_context_rejects_account_id_overflow() {
        let identity = user_identity(AccountType::Doctor, u64::MAX, 456);

        match doctor_auth_context(&identity) {
            Err(TraceError::BadRequest(message)) => {
                assert_eq!(message, "doctor account id is too large");
            }
            Err(_) => panic!("expected bad request for oversized doctor account id"),
            Ok(_) => panic!("expected oversized doctor account id to be rejected"),
        }
    }

    #[test]
    fn doctor_auth_context_rejects_profile_id_overflow() {
        let identity = user_identity(AccountType::Doctor, 123, u64::MAX);

        match doctor_auth_context(&identity) {
            Err(TraceError::BadRequest(message)) => {
                assert_eq!(message, "doctor profile id is too large");
            }
            Err(_) => panic!("expected bad request for oversized doctor profile id"),
            Ok(_) => panic!("expected oversized doctor profile id to be rejected"),
        }
    }

    #[test]
    fn map_error_maps_bad_request_to_http_400() {
        assert_eq!(
            status_for(ConsultationConfigError::BadRequest("invalid".to_string())),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn map_error_maps_doctor_identity_not_provisioned_to_http_409() {
        assert_eq!(
            status_for(ConsultationConfigError::DoctorIdentityNotProvisioned),
            StatusCode::CONFLICT
        );
    }

    #[test]
    fn map_error_maps_repository_error_to_http_500() {
        assert_eq!(
            status_for(ConsultationConfigError::Repository(
                ConsultationConfigRepoError::Database(sqlx::Error::RowNotFound),
            )),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn map_error_maps_event_publish_error_to_http_500() {
        assert_eq!(
            status_for(ConsultationConfigError::EventPublish(anyhow::anyhow!(
                "publish failed"
            ))),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
