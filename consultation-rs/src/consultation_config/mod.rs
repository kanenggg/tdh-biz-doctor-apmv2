pub mod handler;
pub mod model;
pub mod repo;
pub mod service;
pub mod validate;
pub mod window;

use std::sync::Arc;

use axum::{Router, middleware};

use crate::common::infrastructure::Infrastructure;

pub fn router() -> Router<handler::AppState> {
    handler::router().layer(middleware::from_fn(crate::common::auth_middleware))
}

pub fn bootstrap(infra: &Infrastructure) -> handler::AppState {
    let repo: Arc<dyn repo::ConsultationConfigRepo> =
        Arc::new(repo::ConsultationConfigRepoPsql::new(infra.db_pool.clone()));
    let service = service::ConsultationConfigService::new(repo);

    handler::AppState { service }
}
