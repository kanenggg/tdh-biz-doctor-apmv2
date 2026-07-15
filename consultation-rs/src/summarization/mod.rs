pub mod follow_up_repo;
pub mod handler;
pub mod repo;
pub mod service;
pub mod state;

use axum::Router;

use crate::common::infrastructure::Infrastructure;

pub use crate::protocol::summary_note::{SummarizationError, SummarizationResult};
pub use repo::{CreateSummaryNoteParams, SummaryNoteRepoPsql};
pub use state::AppState;

pub fn router() -> Router<state::AppState> {
    Router::new().merge(handler::router())
}

pub async fn bootstrap(infra: &Infrastructure) -> anyhow::Result<state::AppState> {
    state::bootstrap(infra).await
}
