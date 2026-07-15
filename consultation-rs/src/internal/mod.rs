pub mod handlers;
pub mod repo;
pub mod service;

use std::sync::Arc;

use axum::Router;
use axum::routing::post;

use crate::common::infrastructure::Infrastructure;
use crate::internal::handlers::{AppState, create_appointment, create_confirmed_appointment};
use crate::internal::repo::InternalRepo;
use crate::internal::service::{CreateAppointmentService, CreateConfirmedAppointment};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/v2/internal/create-confirmed-appointment",
            post(create_confirmed_appointment),
        )
        .route("/v2/internal/create-appointment", post(create_appointment))
}

pub fn bootstrap(infra: &Infrastructure) -> AppState {
    let internal_repo = Arc::new(InternalRepo::new(infra.db_pool.clone()));
    let create_confirmed_appointment_service =
        CreateConfirmedAppointment::new(internal_repo.clone());
    let create_appointment_service = CreateAppointmentService::new(internal_repo);
    AppState {
        create_confirmed_appointment_service,
        create_appointment_service,
    }
}
