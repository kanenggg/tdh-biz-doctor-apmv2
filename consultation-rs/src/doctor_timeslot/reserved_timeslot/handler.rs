use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};

use crate::common::{TraceError, internal_error_msg};
use crate::doctor_timeslot::reserved_timeslot::model::{
    ReservedTimeslotsQuery, ReservedTimeslotsResponse,
};
use crate::doctor_timeslot::reserved_timeslot::service::ReservedTimeslotsError;
use crate::doctor_timeslot::state::AppState;

const RESERVED_TIMESLOTS_PATH: &str = "/v2/internal/doctor-timeslot/reserved-timeslots";

#[utoipa::path(
    get,
    path = RESERVED_TIMESLOTS_PATH,
    tag = "doctor-timeslot",
    params(ReservedTimeslotsQuery),
    responses(
        (status = 200, description = "Active reserved timeslots for the doctor in the datetime range", body = ReservedTimeslotsResponse),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_reserved_timeslots(
    State(state): State<AppState>,
    Query(query): Query<ReservedTimeslotsQuery>,
) -> Result<Json<ReservedTimeslotsResponse>, TraceError> {
    tracing::info!(
        doctor_profile_id = query.doctor_profile_id,
        from_datetime = %query.from_datetime,
        to_datetime = %query.to_datetime,
        "doctor_timeslot get_reserved_timeslots start"
    );

    state
        .reserved_timeslots_service
        .get_reserved_timeslots(
            query.doctor_profile_id,
            &query.from_datetime,
            &query.to_datetime,
        )
        .await
        .map(Json)
        .map_err(map_error)
}

fn map_error(error: ReservedTimeslotsError) -> TraceError {
    match error {
        ReservedTimeslotsError::InvalidRequest(message) => TraceError::BadRequest(message),
        ReservedTimeslotsError::Repository(error) => {
            tracing::error!(%error, "doctor_timeslot get_reserved_timeslots failed");
            internal_error_msg("Failed to fetch reserved timeslots")
        }
    }
}

pub fn router() -> Router<AppState> {
    Router::new().route(RESERVED_TIMESLOTS_PATH, get(get_reserved_timeslots))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use axum::{http::StatusCode, response::IntoResponse};
    use uuid::Uuid;

    use crate::doctor_timeslot::get_timeslot::repo::{
        DoctorScheduleCandidate, DoctorTimeslotConfigSnapshot, DoctorTimeslotIdentity,
        GetDoctorTimeslotRepo, ReservedWindow,
    };
    use crate::doctor_timeslot::get_timeslot::service::GetDoctorTimeslotService;
    use crate::doctor_timeslot::reserved_timeslot::model::ReserveTimeSlot;
    use crate::doctor_timeslot::reserved_timeslot::repo::ReservedTimeslotsRepo;
    use crate::doctor_timeslot::reserved_timeslot::service::ReservedTimeslotsService;

    #[derive(Default)]
    struct RecordingReservedTimeslotsRepo {
        calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl ReservedTimeslotsRepo for RecordingReservedTimeslotsRepo {
        async fn find_reserved_timeslots_by_doctor_profile(
            &self,
            _doctor_profile_id: i32,
            _day_start: i64,
            _day_end: i64,
        ) -> Result<Vec<ReserveTimeSlot>, anyhow::Error> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        }
    }

    struct UnusedGetDoctorTimeslotRepo;

    #[async_trait::async_trait]
    impl GetDoctorTimeslotRepo for UnusedGetDoctorTimeslotRepo {
        async fn resolve_doctor_identity(
            &self,
            _doctor_account_id: i64,
            _doctor_profile_id: i64,
        ) -> Result<Option<DoctorTimeslotIdentity>, anyhow::Error> {
            unreachable!("get timeslot repo is not used by this handler test")
        }

        async fn resolve_doctor_identity_by_doctor_id(
            &self,
            _doctor_id: Uuid,
        ) -> Result<Option<DoctorTimeslotIdentity>, anyhow::Error> {
            unreachable!("get timeslot repo is not used by this handler test")
        }

        async fn list_schedule_available_doctors(
            &self,
        ) -> Result<Vec<DoctorScheduleCandidate>, anyhow::Error> {
            unreachable!("get timeslot repo is not used by this handler test")
        }

        async fn get_config_snapshot(
            &self,
            _doctor_id: Uuid,
        ) -> Result<DoctorTimeslotConfigSnapshot, anyhow::Error> {
            unreachable!("get timeslot repo is not used by this handler test")
        }

        async fn list_reserved_windows(
            &self,
            _doctor_profile_id: i64,
            _from_epoch: i64,
            _to_epoch: i64,
        ) -> Result<Vec<ReservedWindow>, anyhow::Error> {
            unreachable!("get timeslot repo is not used by this handler test")
        }
    }

    fn test_state(reserved_timeslots_repo: Arc<RecordingReservedTimeslotsRepo>) -> AppState {
        AppState {
            get_timeslot_service: GetDoctorTimeslotService::new(
                Arc::new(UnusedGetDoctorTimeslotRepo),
                30 * 60,
            ),
            reserved_timeslots_service: ReservedTimeslotsService::new(reserved_timeslots_repo),
        }
    }

    #[test]
    fn maps_invalid_request_to_bad_request() {
        match map_error(ReservedTimeslotsError::InvalidRequest(
            "bad range".to_string(),
        )) {
            TraceError::BadRequest(message) => assert_eq!(message, "bad range"),
            _ => panic!("invalid request should map to bad request"),
        }
    }

    #[test]
    fn maps_repository_error_to_internal_error() {
        match map_error(ReservedTimeslotsError::Repository(anyhow::anyhow!(
            "db down"
        ))) {
            TraceError::InternalError(message) => {
                assert_eq!(message, "Failed to fetch reserved timeslots")
            }
            _ => panic!("repository error should map to internal error"),
        }
    }

    #[tokio::test]
    async fn invalid_datetime_returns_bad_request_without_repo_lookup() {
        let reserved_timeslots_repo = Arc::new(RecordingReservedTimeslotsRepo::default());
        let state = test_state(reserved_timeslots_repo.clone());

        let response = get_reserved_timeslots(
            State(state),
            Query(ReservedTimeslotsQuery {
                doctor_profile_id: 84,
                from_datetime: "not-a-datetime".to_string(),
                to_datetime: "2026-06-18T17:00:00Z".to_string(),
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(reserved_timeslots_repo.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn invalid_range_returns_bad_request_without_repo_lookup() {
        let reserved_timeslots_repo = Arc::new(RecordingReservedTimeslotsRepo::default());
        let state = test_state(reserved_timeslots_repo.clone());

        let response = get_reserved_timeslots(
            State(state),
            Query(ReservedTimeslotsQuery {
                doctor_profile_id: 84,
                from_datetime: "2026-06-18T17:00:00Z".to_string(),
                to_datetime: "2026-06-18T17:00:00Z".to_string(),
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(reserved_timeslots_repo.calls.load(Ordering::SeqCst), 0);
    }
}
