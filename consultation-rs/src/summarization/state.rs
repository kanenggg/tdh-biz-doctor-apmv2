use std::sync::Arc;

use crate::common::infrastructure::Infrastructure;
use crate::summarization::follow_up_repo::FollowUpRepoPsql;
use crate::summarization::repo::SummaryNoteRepoPsql;
use crate::summarization::service::SummaryNoteService;
use crate::sys::crypto::kms::GcpKmsService;

#[derive(Clone)]
pub struct AppState {
    pub summarization_service: SummaryNoteService,
}

pub async fn bootstrap(infra: &Infrastructure) -> anyhow::Result<AppState> {
    let repo = Arc::new(SummaryNoteRepoPsql::new(infra.db_pool.clone()));
    let follow_up_repo = Arc::new(FollowUpRepoPsql::new(infra.db_pool.clone()));
    let kms = Arc::new(GcpKmsService::new().await?);
    let kms_key_name = infra.config.google_cloud.kms.doctor_note.clone();

    let summarization_service = SummaryNoteService::new(
        repo,
        follow_up_repo,
        infra.event_publisher.clone(),
        kms,
        kms_key_name,
    );

    Ok(AppState {
        summarization_service,
    })
}
