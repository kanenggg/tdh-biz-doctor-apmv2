pub mod consultation_summary;
pub mod get_detail;
pub mod hold;
pub mod list;
/// Compatibility route adapter for the internal reserved-timeslots read.
pub mod reserved_timeslots;
pub mod state;

pub(crate) mod types;

use axum::Router;
use std::sync::Arc;

use crate::appointment::consultation_summary::repo::ConsultationSummaryRepoPsql;
use crate::appointment::consultation_summary::service::ConsultationSummaryService;
use crate::appointment::get_detail::repo::GetAppointmentDetailRepoPsql;
use crate::appointment::get_detail::service::GetAppointmentDetailService;
use crate::appointment::list::repo::ListAppointmentsRepoPsql;
use crate::appointment::list::service::ListAppointmentsService;
use crate::appointment::state::AppState;
use crate::common::infrastructure::Infrastructure;
use crate::sys::crypto::kms::{GcpKmsService, Kms};

#[cfg(test)]
const PUBLIC_APPOINTMENT_ROUTES: &[&str] = &[];
#[cfg(test)]
const INTERNAL_APPOINTMENT_ROUTES: &[&str] = &[
    // Preferred internal appointment read routes use access-before-version.
    "/internal/v1/appointment/{bookingId}",
    "/internal/v1/appointments/{bookingId}",
    "/internal/v1/appointment/{bookingId}/consultation-summary",
    "/internal/v1/appointments",
    "/internal/v1/appointment/reserved-timeslots",
    // Legacy aliases retained during cleanup from the legacy /v2/internal convention.
    "/v2/internal/appointment/{bookingId}",
    "/v2/internal/appointments/{bookingId}",
    "/v2/internal/appointment/{bookingId}/consultation-summary",
    "/v2/internal/appointments",
    "/v2/internal/appointment/reserved-timeslots",
];

pub fn router() -> Router<AppState> {
    // The legacy appointment reserve-timeslot module stays compiled for compatibility, but its
    // public /v1 and /v2 reserve routes are no longer mounted here. Public reservations are owned
    // by Appointment Hold; `booking` remains route-adapter vocabulary (/v1/booking, legacy /v2/booking).
    let public = Router::new().layer(axum::middleware::from_fn(crate::common::auth_middleware));

    // No auth middleware on the internal branch — these endpoints are
    // /internal/v1/appointment/{bookingId} plus legacy /v2/internal aliases, reachable only from inside
    // the cluster by network policy. Do NOT add auth_middleware to the
    // combined router below; apply layers per sub-router instead so
    // the public and internal branches stay decoupled.
    let internal = get_detail::handler::router()
        .merge(consultation_summary::handler::router())
        .merge(list::handler::router())
        .merge(reserved_timeslots::handler::router());
    Router::new().merge(public).merge(internal)
}

pub async fn bootstrap(infra: &Infrastructure) -> anyhow::Result<AppState> {
    let get_detail_repo = Arc::new(GetAppointmentDetailRepoPsql::new(infra.db_pool.clone()));

    if infra.config.google_cloud.kms.prescreen.is_empty() {
        tracing::warn!(
            "google_cloud.kms.prescreen is empty; prescreen decryption will fail at runtime"
        );
    }
    let kms: Arc<dyn Kms> = Arc::new(GcpKmsService::new().await?);
    let kms_key_name = infra.config.google_cloud.kms.prescreen.clone();

    let get_appointment_detail_service =
        GetAppointmentDetailService::new(get_detail_repo, kms.clone(), kms_key_name);

    // Consultation summary detail decrypts the doctor summary note, which the summarization
    // service encrypts with the `doctor_note` key — must match to decrypt.
    if infra.config.google_cloud.kms.doctor_note.is_empty() {
        tracing::warn!(
            "google_cloud.kms.doctor_note is empty; summary note decryption will fail at runtime"
        );
    }
    let consultation_summary_repo =
        Arc::new(ConsultationSummaryRepoPsql::new(infra.db_pool.clone()));
    let consultation_summary_service = ConsultationSummaryService::new(
        consultation_summary_repo,
        kms,
        infra.config.google_cloud.kms.doctor_note.clone(),
    );

    let list_appointments_repo = Arc::new(ListAppointmentsRepoPsql::new(infra.db_pool.clone()));
    let list_appointments_service = ListAppointmentsService::new(list_appointments_repo);

    let reserved_timeslots_repo =
        crate::doctor_timeslot::reserved_timeslot::repo::ReservedTimeslotsRepoPsql::new(
            infra.db_pool.clone(),
        );
    let reserved_timeslots_service =
        crate::doctor_timeslot::reserved_timeslot::service::ReservedTimeslotsService::new(
            Arc::new(reserved_timeslots_repo),
        );

    Ok(AppState {
        get_appointment_detail_service,
        consultation_summary_service,
        list_appointments_service,
        reserved_timeslots_service,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_appointment_routes_are_empty() {
        assert!(PUBLIC_APPOINTMENT_ROUTES.is_empty());
    }

    #[test]
    fn internal_appointment_routes_expose_preferred_internal_v1_read_aliases() {
        assert!(INTERNAL_APPOINTMENT_ROUTES.contains(&"/internal/v1/appointment/{bookingId}"));
        assert!(INTERNAL_APPOINTMENT_ROUTES.contains(&"/internal/v1/appointments/{bookingId}"));
        assert!(
            INTERNAL_APPOINTMENT_ROUTES
                .contains(&"/internal/v1/appointment/{bookingId}/consultation-summary")
        );
        assert!(INTERNAL_APPOINTMENT_ROUTES.contains(&"/internal/v1/appointments"));
        assert!(
            INTERNAL_APPOINTMENT_ROUTES.contains(&"/internal/v1/appointment/reserved-timeslots")
        );
    }

    #[test]
    fn internal_appointment_routes_keep_legacy_v2_internal_read_aliases() {
        // Legacy aliases remain during cleanup from the legacy /v2/internal convention.
        assert!(INTERNAL_APPOINTMENT_ROUTES.contains(&"/v2/internal/appointment/{bookingId}"));
        assert!(INTERNAL_APPOINTMENT_ROUTES.contains(&"/v2/internal/appointments/{bookingId}"));
        assert!(
            INTERNAL_APPOINTMENT_ROUTES
                .contains(&"/v2/internal/appointment/{bookingId}/consultation-summary")
        );
        assert!(INTERNAL_APPOINTMENT_ROUTES.contains(&"/v2/internal/appointments"));
        assert!(
            INTERNAL_APPOINTMENT_ROUTES.contains(&"/v2/internal/appointment/reserved-timeslots")
        );
    }
}
