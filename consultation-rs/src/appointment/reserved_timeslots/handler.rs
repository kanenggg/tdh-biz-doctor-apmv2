use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};

use crate::appointment::state::AppState;
use crate::common::{TraceError, internal_error_msg};
use crate::doctor_timeslot::reserved_timeslot::{
    model::{ReservedTimeslotsQuery, ReservedTimeslotsResponse},
    service::ReservedTimeslotsError,
};

#[utoipa::path(
    get,
    path = "/internal/v1/appointment/reserved-timeslots",
    tag = "appointment",
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
        "get_reserved_timeslots start"
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
            tracing::error!(%error, "get_reserved_timeslots failed");
            internal_error_msg("Failed to fetch reserved timeslots")
        }
    }
}

const PREFERRED_INTERNAL_RESERVED_TIMESLOTS_ROUTE: &str =
    "/internal/v1/appointment/reserved-timeslots";

// Legacy alias retained during cleanup from the legacy /v2/internal convention.
const LEGACY_INTERNAL_RESERVED_TIMESLOTS_ROUTE: &str =
    "/v2/internal/appointment/reserved-timeslots";

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            PREFERRED_INTERNAL_RESERVED_TIMESLOTS_ROUTE,
            get(get_reserved_timeslots),
        )
        .route(
            LEGACY_INTERNAL_RESERVED_TIMESLOTS_ROUTE,
            get(get_reserved_timeslots),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use axum::{http::StatusCode, response::IntoResponse};

    use crate::appointment::consultation_summary::repo::{
        ConsultationSummaryRepo, ConsultationSummaryRow,
    };
    use crate::appointment::consultation_summary::service::ConsultationSummaryService;
    use crate::appointment::get_detail::repo::{AppointmentDetailRow, GetAppointmentDetailRepo};
    use crate::appointment::get_detail::service::GetAppointmentDetailService;
    use crate::appointment::list::repo::{AppointmentListRow, ListAppointmentsRepo};
    use crate::appointment::list::service::ListAppointmentsService;
    use crate::common::tdh_protocol::consultation::ConsultationEvent;
    use crate::doctor_timeslot::configuration_events::model::DoctorTimeslotConfigChangedEvent;
    use crate::doctor_timeslot::reserved_timeslot::model::ReserveTimeSlot;
    use crate::doctor_timeslot::reserved_timeslot::repo::ReservedTimeslotsRepo;
    use crate::doctor_timeslot::reserved_timeslot::service::ReservedTimeslotsService;
    use crate::infra::event::EventPublisher;
    use crate::sys::crypto::kms::{Kms, KmsResult};

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

    struct UnusedEventPublisher;

    #[async_trait::async_trait]
    impl EventPublisher for UnusedEventPublisher {
        async fn publish_consultation_event(
            &self,
            _event: ConsultationEvent,
        ) -> Result<(), anyhow::Error> {
            unreachable!("event publisher is not used by this handler test")
        }

        async fn publish_doctor_timeslot_config_changed_event(
            &self,
            _event: DoctorTimeslotConfigChangedEvent,
        ) -> Result<(), anyhow::Error> {
            unreachable!("event publisher is not used by this handler test")
        }
    }

    struct UnusedGetDetailRepo;

    #[async_trait::async_trait]
    impl GetAppointmentDetailRepo for UnusedGetDetailRepo {
        async fn get_appointment_detail(
            &self,
            _booking_id: &str,
        ) -> Result<Option<AppointmentDetailRow>, anyhow::Error> {
            unreachable!("appointment detail repo is not used by this handler test")
        }
    }

    struct UnusedConsultationSummaryRepo;

    #[async_trait::async_trait]
    impl ConsultationSummaryRepo for UnusedConsultationSummaryRepo {
        async fn get_consultation_summary(
            &self,
            _booking_id: &str,
        ) -> Result<Option<ConsultationSummaryRow>, anyhow::Error> {
            unreachable!("consultation summary repo is not used by this handler test")
        }
    }

    struct UnusedListAppointmentsRepo;

    #[async_trait::async_trait]
    impl ListAppointmentsRepo for UnusedListAppointmentsRepo {
        async fn list_fulfilled_appointments(
            &self,
            _patient_account_id: i32,
            _patient_profile_id: Option<i32>,
        ) -> Result<Vec<AppointmentListRow>, anyhow::Error> {
            unreachable!("appointment list repo is not used by this handler test")
        }
    }

    struct NoopKms;

    #[async_trait::async_trait]
    impl Kms for NoopKms {
        async fn encrypt(&self, plaintext: &[u8], _key_name: &str) -> KmsResult<Vec<u8>> {
            Ok(plaintext.to_vec())
        }

        async fn decrypt(&self, ciphertext: &[u8], _key_name: &str) -> KmsResult<Vec<u8>> {
            Ok(ciphertext.to_vec())
        }
    }

    fn test_state(reserved_timeslots_repo: Arc<RecordingReservedTimeslotsRepo>) -> AppState {
        let kms = Arc::new(NoopKms);

        AppState {
            get_appointment_detail_service: GetAppointmentDetailService::new(
                Arc::new(UnusedGetDetailRepo),
                kms.clone(),
                String::new(),
            ),
            consultation_summary_service: ConsultationSummaryService::new(
                Arc::new(UnusedConsultationSummaryRepo),
                kms,
                String::new(),
            ),
            list_appointments_service: ListAppointmentsService::new(Arc::new(
                UnusedListAppointmentsRepo,
            )),
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
}
