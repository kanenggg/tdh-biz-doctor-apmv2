use std::sync::Arc;

use crate::common::tdh_protocol::internal::{
    CreateAppointmentRequest, CreateAppointmentResult, CreateConfirmedInstantAppointmentRequest,
};
use crate::internal::repo::InternalRepo;

#[derive(Clone)]
pub struct CreateConfirmedAppointment {
    internal_repo: Arc<InternalRepo>,
}

impl CreateConfirmedAppointment {
    pub fn new(internal_repo: Arc<InternalRepo>) -> Self {
        Self { internal_repo }
    }

    pub async fn create_confirmed_appointment(
        &self,
        req: CreateConfirmedInstantAppointmentRequest,
    ) -> Result<i64, anyhow::Error> {
        self.internal_repo.add_confirmed_appointment(req).await
    }
}

#[derive(Clone)]
pub struct CreateAppointmentService {
    internal_repo: Arc<InternalRepo>,
}

impl CreateAppointmentService {
    pub fn new(internal_repo: Arc<InternalRepo>) -> Self {
        Self { internal_repo }
    }

    pub async fn create_appointment(
        &self,
        req: CreateAppointmentRequest,
    ) -> Result<CreateAppointmentResult, anyhow::Error> {
        self.internal_repo.create_appointment(req).await
    }
}
