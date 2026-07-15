pub mod common;
pub(crate) mod end_session;
pub(crate) mod facial_upload;
pub(crate) mod patient_verification;
pub mod session_info;
pub mod state;

use axum::Router;
use std::sync::Arc;

use crate::common::infrastructure::Infrastructure;
use crate::consultation::state::AppState;

pub use end_session::service::EndSessionService;
pub use facial_upload::service::FacialUploadService;
pub use patient_verification::service::PatientVerificationService;
pub use session_info::repo::GetOrCreateSessionRepoPsql;
pub use session_info::service::GetOrCreateConsultSessionService;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(session_info::handler::router())
        .merge(end_session::handler::router())
        .merge(patient_verification::handler::router())
        .merge(facial_upload::handler::router())
        .layer(axum::middleware::from_fn(crate::common::auth_middleware))
}

pub fn bootstrap(infra: &Infrastructure) -> AppState {
    let session_repo = session_info::repo::GetOrCreateSessionRepoPsql::new(infra.db_pool.clone());
    let session_service = GetOrCreateConsultSessionService::new(
        Arc::new(session_repo),
        infra.twilio_client.clone(),
        infra.event_publisher.clone(),
        infra.config.clone(),
    );

    let facial_upload_repo = Arc::new(facial_upload::repo::FacialUploadRepoPsql::new(
        infra.db_pool.clone(),
    ));
    let facial_upload_service = FacialUploadService::new(
        facial_upload_repo.clone(),
        infra.gcs_client.clone(),
        format!(
            "projects/_/buckets/{}",
            infra.config.google_cloud.facial_upload_bucket
        ),
    );

    let end_session_repo = Arc::new(end_session::repo::EndSessionRepoPsql::new(
        infra.db_pool.clone(),
    ));
    let end_session_service = EndSessionService::new(
        infra.event_publisher.clone(),
        infra.twilio_client.clone(),
        end_session_repo,
    );

    let patient_verification_repo = Arc::new(
        patient_verification::repo::PatientVerificationRepoPsql::new(infra.db_pool.clone()),
    );
    let patient_verification_service =
        PatientVerificationService::new(patient_verification_repo, infra.event_publisher.clone());

    let rtdb_token_issuer =
        session_info::rtdb_access::RtdbCustomTokenIssuer::from_config(&infra.config.rtdb_access);

    AppState {
        session_service,
        rtdb_token_issuer,
        facial_upload_service,
        end_session_service,
        patient_verification_service,
    }
}
