pub mod configuration_events;
pub mod get_timeslot;
pub mod reserved_timeslot;
pub mod state;

use std::sync::Arc;

use axum::{Router, middleware};

use crate::common::infrastructure::Infrastructure;
use get_timeslot::repo::GetDoctorTimeslotRepoPsql;
use get_timeslot::service::GetDoctorTimeslotService;

pub fn router() -> Router<state::AppState> {
    let public =
        get_timeslot::handler::router().layer(middleware::from_fn(crate::common::auth_middleware));
    let internal = reserved_timeslot::handler::router();

    Router::new().merge(public).merge(internal)
}

pub fn bootstrap(infra: &Infrastructure) -> state::AppState {
    let repo = Arc::new(GetDoctorTimeslotRepoPsql::new(infra.db_pool.clone()));
    let get_timeslot_service = GetDoctorTimeslotService::new_with_v2_snapshot(
        repo,
        infra.config.consultation_duration().as_secs() as i64,
        infra.config.doctor_service_projection.require_v2_snapshot,
    );

    let reserved_timeslots_repo =
        reserved_timeslot::repo::ReservedTimeslotsRepoPsql::new(infra.db_pool.clone());
    let reserved_timeslots_service = reserved_timeslot::service::ReservedTimeslotsService::new(
        Arc::new(reserved_timeslots_repo),
    );

    state::AppState {
        get_timeslot_service,
        reserved_timeslots_service,
    }
}
