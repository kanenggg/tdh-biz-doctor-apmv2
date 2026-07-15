use crate::appointment_hold::repo::AppointmentHoldExpiryRepo;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub(crate) enum AppointmentHoldExpiryError {
    #[error("Appointment Hold expiry database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub(crate) struct AppointmentHoldExpiryService {
    repo: Arc<dyn AppointmentHoldExpiryRepo>,
    topic: String,
}
impl AppointmentHoldExpiryService {
    pub(crate) fn new(repo: Arc<dyn AppointmentHoldExpiryRepo>, topic: String) -> Self {
        Self { repo, topic }
    }
    pub(crate) async fn expire_due_holds(
        &self,
        batch_size: i32,
    ) -> Result<usize, AppointmentHoldExpiryError> {
        Ok(self.repo.expire_due_holds(batch_size, &self.topic).await?)
    }
}
