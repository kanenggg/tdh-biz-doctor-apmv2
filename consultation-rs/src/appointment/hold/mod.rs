//! Appointment Hold is the canonical owner of hold creation, hold state, and internal hold cancellation.
//!
//! The mounted HTTP routes intentionally keep `booking` path vocabulary (`/v1/booking`,
//! `/internal/v1/booking`, and legacy `/v2/...`) as route-adapter compatibility only.

pub mod create;
pub mod handler;
pub mod model;
pub mod repo;
pub mod service;
pub mod state;

use axum::{Router, middleware};
use std::sync::Arc;

use crate::appointment::hold::create::AppointmentHoldService;
use crate::appointment::hold::handler::PublicBookingRateLimiter;
use crate::appointment::hold::repo::{
    AppointmentHoldPsql, BookingRepoPsql, DoctorHoldProfileCache,
};
use crate::appointment::hold::service::BookingService;
use crate::common::infrastructure::Infrastructure;

pub fn router() -> Router<state::AppState> {
    let public = Router::new()
        .merge(handler::public_router())
        .layer(middleware::from_fn(crate::common::auth_middleware));

    let internal = handler::internal_router();

    Router::new().merge(public).merge(internal)
}

pub fn bootstrap(infra: &Infrastructure) -> state::AppState {
    let doctor_profile_cache = DoctorHoldProfileCache::new(infra.redis_pool.clone());
    let hold_repo = AppointmentHoldPsql::new(
        infra.db_pool.clone(),
        infra.config.booking.pubsub_topic.clone(),
    );
    let appointment_hold_service = AppointmentHoldService::new(
        Arc::new(hold_repo),
        Arc::new(doctor_profile_cache),
        infra.config.booking.reservation_ttl_seconds,
    );

    let booking_repo = BookingRepoPsql::with_v1_topic(
        infra.db_pool.clone(),
        Some(infra.config.booking.pubsub_topic.clone()),
    );
    let booking_service =
        BookingService::new(Arc::new(booking_repo), infra.event_publisher.clone());

    state::AppState {
        appointment_hold_service,
        booking_service,
        public_booking_rate_limiter: PublicBookingRateLimiter::default_public_policy(),
    }
}
