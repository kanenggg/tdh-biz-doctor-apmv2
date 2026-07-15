pub mod outbox;
pub mod publisher;

use crate::common::tdh_protocol::consultation::ConsultationEvent;
use crate::doctor_timeslot::configuration_events::model::DoctorTimeslotConfigChangedEvent;
pub use outbox::OutboxEventPublisher;
pub use publisher::{NoOpEventPublisher, PubSubEventPublisher};
use uuid::Uuid;

#[async_trait::async_trait]
pub trait EventPublisher: Send + Sync {
    // async fn publish_session_created(&self, event: SessionMessage) -> Result<(), anyhow::Error>;

    async fn publish_consultation_event(
        &self,
        event: ConsultationEvent,
    ) -> Result<(), anyhow::Error>;

    async fn publish_consultation_event_with_id(
        &self,
        _event_id: Uuid,
        event: ConsultationEvent,
    ) -> Result<(), anyhow::Error> {
        self.publish_consultation_event(event).await
    }

    async fn publish_doctor_timeslot_config_changed_event(
        &self,
        event: DoctorTimeslotConfigChangedEvent,
    ) -> Result<(), anyhow::Error>;
}
