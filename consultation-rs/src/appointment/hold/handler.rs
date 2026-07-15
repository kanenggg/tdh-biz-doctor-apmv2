use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    routing::{get, post},
};

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use crate::appointment::hold::create::HoldError;
use crate::appointment::hold::model::{
    BookingCancelResponse, BookingResponse, BookingStateResponse, CreateAppointmentHold,
    InternalBookingRequest, PublicBookingRequest,
};
use crate::appointment::hold::service::BookingError;
use crate::appointment::hold::state::AppState;
use crate::common::tdh_protocol::iam::user_identity::{AccountType, UserIdentity};
use crate::common::{TraceError, internal_error_msg};

const PUBLIC_BOOKING_RATE_LIMIT_MAX_REQUESTS: usize = 5;
const PUBLIC_BOOKING_RATE_LIMIT_WINDOW_SECONDS: i64 = 60;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PublicBookingRateLimitKey {
    patient_account_id: u64,
    patient_profile_id: u64,
    doctor_id: i32,
}

impl PublicBookingRateLimitKey {
    pub fn new(patient_account_id: u64, patient_profile_id: u64, doctor_id: i32) -> Self {
        Self {
            patient_account_id,
            patient_profile_id,
            doctor_id,
        }
    }
}

#[derive(Clone)]
pub struct PublicBookingRateLimiter {
    max_requests: usize,
    window_seconds: i64,
    hits: Arc<Mutex<HashMap<PublicBookingRateLimitKey, VecDeque<i64>>>>,
}

impl PublicBookingRateLimiter {
    pub fn new(max_requests: usize, window_seconds: i64) -> Self {
        Self {
            max_requests,
            window_seconds,
            hits: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn default_public_policy() -> Self {
        Self::new(
            PUBLIC_BOOKING_RATE_LIMIT_MAX_REQUESTS,
            PUBLIC_BOOKING_RATE_LIMIT_WINDOW_SECONDS,
        )
    }

    pub fn try_acquire(&self, key: PublicBookingRateLimitKey) -> bool {
        self.try_acquire_at(key, jiff::Timestamp::now().as_second())
    }

    pub fn try_acquire_at(&self, key: PublicBookingRateLimitKey, now_epoch: i64) -> bool {
        let window_start = now_epoch - self.window_seconds;
        let mut hits = self
            .hits
            .lock()
            .expect("public booking rate limiter mutex poisoned");
        let entries = hits.entry(key).or_default();
        while entries
            .front()
            .is_some_and(|seen_at| *seen_at <= window_start)
        {
            entries.pop_front();
        }
        if entries.len() >= self.max_requests {
            return false;
        }
        entries.push_back(now_epoch);
        true
    }
}

#[utoipa::path(
    post,
    path = "/v1/booking",
    tag = "booking",
    request_body = PublicBookingRequest,
    responses(
        (status = 200, description = "Booking reservation created", body = BookingResponse),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Internal server error")
    ),
    security(("ApiKeyAuth" = []))
)]
pub async fn public_booking(
    State(state): State<AppState>,
    Extension(user_identity): Extension<UserIdentity>,
    Json(request): Json<PublicBookingRequest>,
) -> Result<Json<BookingResponse>, TraceError> {
    let key = PublicBookingRateLimitKey::new(
        user_identity.account_id,
        user_identity.user_profile_id,
        request.reserve.doctor_id,
    );
    if !state.public_booking_rate_limiter.try_acquire(key) {
        return Err(TraceError::Conflict(
            "Public booking rate limit exceeded".to_string(),
        ));
    }

    create_hold_with_user_identity(state, user_identity, request.reserve).await
}

#[utoipa::path(
    post,
    path = "/internal/v1/booking",
    tag = "booking",
    request_body = InternalBookingRequest,
    responses(
        (status = 200, description = "Internal booking reservation created", body = BookingResponse),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn internal_booking(
    State(state): State<AppState>,
    Json(request): Json<InternalBookingRequest>,
) -> Result<Json<BookingResponse>, TraceError> {
    let user_identity = UserIdentity {
        account_id: request.patient_identity.account_id,
        account_type: AccountType::Patient,
        user_profile_id: request.patient_identity.user_profile_id,
        user_main_profile_id: request.patient_identity.user_profile_id,
        tenant_id: request.patient_identity.tenant_id,
        oidc_user_id: request.patient_identity.oidc_user_id,
        legacy_data: None,
    };

    create_hold_with_user_identity(state, user_identity, request.reserve).await
}

async fn create_hold_with_user_identity(
    state: AppState,
    user_identity: UserIdentity,
    request: crate::common::tdh_protocol::appointment::ReserveRequest,
) -> Result<Json<BookingResponse>, TraceError> {
    match state
        .appointment_hold_service
        .create_hold(user_identity, CreateAppointmentHold::from(request))
        .await
    {
        Ok(response) => Ok(Json(response.into())),
        Err(HoldError::InvalidRequest(message)) => Err(TraceError::BadRequest(message)),
        Err(HoldError::DoctorNotAvailable) => Ok(Json(BookingResponse::from(
            crate::common::tdh_protocol::appointment::ReserveResponse::DoctorNotAvailable,
        ))),
        Err(HoldError::SlotAlreadyBooked) => Ok(Json(BookingResponse::from(
            crate::common::tdh_protocol::appointment::ReserveResponse::SlotAlreadyBooked,
        ))),
        Err(HoldError::Database(e)) => {
            tracing::error!(error = %e, "Appointment Hold creation failed");
            Err(internal_error_msg("Failed to create Appointment Hold"))
        }
        Err(HoldError::Profile(e)) => {
            tracing::error!(error = %e, "Appointment Hold profile lookup failed");
            Err(internal_error_msg("Failed to create Appointment Hold"))
        }
    }
}

#[utoipa::path(
    get,
    path = "/v1/booking/{booking_id}/state",
    tag = "booking",
    params(("booking_id" = String, Path, description = "Booking ID")),
    responses(
        (status = 200, description = "Booking state", body = BookingStateResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Booking not found"),
        (status = 500, description = "Internal server error")
    ),
    security(("ApiKeyAuth" = []))
)]
pub async fn public_booking_state(
    State(state): State<AppState>,
    Extension(user_identity): Extension<UserIdentity>,
    Path(booking_id): Path<String>,
) -> Result<Json<BookingStateResponse>, TraceError> {
    state
        .booking_service
        .get_state(&booking_id, Some(&user_identity))
        .await
        .map(Json)
        .map_err(map_booking_error)
}

#[utoipa::path(
    get,
    path = "/internal/v1/booking/{booking_id}/state",
    tag = "booking",
    params(("booking_id" = String, Path, description = "Booking ID")),
    responses(
        (status = 200, description = "Internal booking state", body = BookingStateResponse),
        (status = 404, description = "Booking not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn internal_booking_state(
    State(state): State<AppState>,
    Path(booking_id): Path<String>,
) -> Result<Json<BookingStateResponse>, TraceError> {
    state
        .booking_service
        .get_state(&booking_id, None)
        .await
        .map(Json)
        .map_err(map_booking_error)
}

#[utoipa::path(
    post,
    path = "/internal/v1/booking/{booking_id}/cancel",
    tag = "booking",
    params(("booking_id" = String, Path, description = "Booking ID")),
    responses(
        (status = 200, description = "Appointment Hold released (legacy cancel route and response)", body = BookingCancelResponse),
        (status = 404, description = "Booking not found"),
        (status = 409, description = "Booking cannot be cancelled"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn internal_cancel_booking(
    State(state): State<AppState>,
    Path(booking_id): Path<String>,
) -> Result<Json<BookingCancelResponse>, TraceError> {
    state
        .booking_service
        .release_hold(&booking_id, None)
        .await
        .map(Json)
        .map_err(map_booking_error)
}

fn map_booking_error(error: BookingError) -> TraceError {
    match error {
        BookingError::NotFound => TraceError::NotFound("Booking not found".to_string()),
        BookingError::Unauthorized => {
            TraceError::Unauthorized("Unauthorized booking access".to_string())
        }
        BookingError::CannotCancel => {
            TraceError::Conflict("Booking cannot be cancelled in its current state".to_string())
        }
        BookingError::Repository(error) => {
            tracing::error!(%error, "booking operation failed");
            internal_error_msg("Booking operation failed")
        }
    }
}

const PUBLIC_BOOKING_CREATE_ROUTES: &[&str] = &["/v1/booking", "/v2/booking"];
const PUBLIC_BOOKING_STATE_ROUTES: &[&str] = &[
    "/v1/booking/{booking_id}/state",
    "/v2/booking/{booking_id}/state",
];
const INTERNAL_BOOKING_CREATE_ROUTES: &[&str] = &["/internal/v1/booking", "/v2/internal/booking"];
const INTERNAL_BOOKING_STATE_ROUTES: &[&str] = &[
    "/internal/v1/booking/{booking_id}/state",
    "/v2/internal/booking/{booking_id}/state",
];
const INTERNAL_BOOKING_CANCEL_ROUTES: &[&str] = &[
    "/internal/v1/booking/{booking_id}/cancel",
    "/v2/internal/booking/{booking_id}/cancel",
];
const PUBLIC_BOOKING_ROUTES: &[&str] = &[
    "/v1/booking",
    "/v1/booking/{booking_id}/state",
    "/v2/booking",
    "/v2/booking/{booking_id}/state",
];
const INTERNAL_BOOKING_ROUTES: &[&str] = &[
    "/internal/v1/booking",
    "/internal/v1/booking/{booking_id}/state",
    "/internal/v1/booking/{booking_id}/cancel",
    "/v2/internal/booking",
    "/v2/internal/booking/{booking_id}/state",
    "/v2/internal/booking/{booking_id}/cancel",
];

pub fn public_router() -> Router<AppState> {
    PUBLIC_BOOKING_CREATE_ROUTES
        .iter()
        .fold(Router::new(), |router, route| {
            router.route(route, post(public_booking))
        })
        .merge(
            PUBLIC_BOOKING_STATE_ROUTES
                .iter()
                .fold(Router::new(), |router, route| {
                    router.route(route, get(public_booking_state))
                }),
        )
}

pub fn internal_router() -> Router<AppState> {
    INTERNAL_BOOKING_CREATE_ROUTES
        .iter()
        .fold(Router::new(), |router, route| {
            router.route(route, post(internal_booking))
        })
        .merge(
            INTERNAL_BOOKING_STATE_ROUTES
                .iter()
                .fold(Router::new(), |router, route| {
                    router.route(route, get(internal_booking_state))
                }),
        )
        .merge(
            INTERNAL_BOOKING_CANCEL_ROUTES
                .iter()
                .fold(Router::new(), |router, route| {
                    router.route(route, post(internal_cancel_booking))
                }),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_booking_routes_exclude_cancel_route() {
        assert!(!PUBLIC_BOOKING_ROUTES.contains(&"/v1/booking/{booking_id}/cancel"));
        assert!(!PUBLIC_BOOKING_ROUTES.contains(&"/v2/booking/{booking_id}/cancel"));
    }

    #[test]
    fn internal_booking_routes_include_cancel_route() {
        assert!(INTERNAL_BOOKING_ROUTES.contains(&"/internal/v1/booking/{booking_id}/cancel"));
        assert!(INTERNAL_BOOKING_ROUTES.contains(&"/v2/internal/booking/{booking_id}/cancel"));
    }

    #[test]
    fn booking_routes_prefer_access_before_version_and_keep_legacy_aliases() {
        assert_eq!(PUBLIC_BOOKING_CREATE_ROUTES[0], "/v1/booking");
        assert!(PUBLIC_BOOKING_CREATE_ROUTES.contains(&"/v2/booking"));
        assert_eq!(INTERNAL_BOOKING_CREATE_ROUTES[0], "/internal/v1/booking");
        assert!(INTERNAL_BOOKING_CREATE_ROUTES.contains(&"/v2/internal/booking"));
        assert!(
            !PUBLIC_BOOKING_ROUTES
                .iter()
                .any(|route| route.starts_with("/public/"))
        );
    }

    #[test]
    fn public_booking_rate_limiter_rejects_over_limit_within_window() {
        let limiter = PublicBookingRateLimiter::new(2, 60);
        let key = PublicBookingRateLimitKey::new(1, 2, 10);

        assert!(limiter.try_acquire_at(key.clone(), 100));
        assert!(limiter.try_acquire_at(key.clone(), 101));
        assert!(!limiter.try_acquire_at(key.clone(), 102));
        assert!(limiter.try_acquire_at(key, 161));
    }

    #[test]
    fn public_booking_rate_limit_key_scopes_patient_and_doctor() {
        let limiter = PublicBookingRateLimiter::new(1, 60);
        assert!(limiter.try_acquire_at(PublicBookingRateLimitKey::new(1, 2, 10), 100));
        assert!(limiter.try_acquire_at(PublicBookingRateLimitKey::new(1, 2, 11), 101));
        assert!(limiter.try_acquire_at(PublicBookingRateLimitKey::new(1, 3, 10), 102));
    }

    #[test]
    fn maps_not_found_to_not_found_response() {
        match map_booking_error(BookingError::NotFound) {
            TraceError::NotFound(message) => assert_eq!(message, "Booking not found"),
            _ => panic!("not found should map to not found"),
        }
    }

    #[test]
    fn maps_unauthorized_to_unauthorized_response() {
        match map_booking_error(BookingError::Unauthorized) {
            TraceError::Unauthorized(message) => assert_eq!(message, "Unauthorized booking access"),
            _ => panic!("unauthorized should map to unauthorized"),
        }
    }

    #[test]
    fn maps_cannot_cancel_to_conflict_response() {
        match map_booking_error(BookingError::CannotCancel) {
            TraceError::Conflict(message) => {
                assert_eq!(message, "Booking cannot be cancelled in its current state")
            }
            _ => panic!("cannot cancel should map to conflict"),
        }
    }

    #[test]
    fn maps_repository_error_to_internal_error_response() {
        match map_booking_error(BookingError::Repository(anyhow::anyhow!("db down"))) {
            TraceError::InternalError(message) => assert_eq!(message, "Booking operation failed"),
            _ => panic!("repository error should map to internal error"),
        }
    }
}
