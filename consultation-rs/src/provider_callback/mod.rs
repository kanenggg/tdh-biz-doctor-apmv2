pub mod handler;
pub mod repo;
pub mod service;
pub mod state;

use std::sync::Arc;

use axum::Router;

use crate::common::infrastructure::Infrastructure;

pub fn router() -> Router<state::AppState> {
    handler::router()
}

pub fn bootstrap(infra: &Infrastructure) -> state::AppState {
    let repo = Arc::new(repo::ProviderCallbackRepoPsql::new(infra.db_pool.clone()));
    let service = service::ProviderCallbackService::new(repo, infra.event_publisher.clone());
    state::AppState {
        service,
        twilio_auth_token: infra.config.twilio.auth_token.clone(),
        twilio_callback_url: infra.config.twilio.callback_url.clone(),
    }
}
