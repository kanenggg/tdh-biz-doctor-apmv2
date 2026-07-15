use std::sync::Arc;

use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::EventPublisher;
use crate::common::tdh_protocol::consultation::{
    ConsultationEvent, PostSessionMessage, PreSessionMessage, SessionMessage,
};
use crate::doctor_timeslot::configuration_events::model::DoctorTimeslotConfigChangedEvent;

const DIRECT_PUBLISH_LOCK_SECONDS: i32 = 60;

const ENQUEUE_DIRECT_PUBLISH_SQL: &str = r#"
INSERT INTO v2.event_outbox (
    event_id,
    topic,
    event_type,
    aggregate_id,
    payload,
    publication_status,
    locked_until
) VALUES ($1, $2, $3, $4, $5, 'PENDING', NOW() + ($6::integer * INTERVAL '1 second'))
"#;

const ENQUEUE_PENDING_SQL: &str = r#"
INSERT INTO v2.event_outbox (
    event_id,
    topic,
    event_type,
    aggregate_id,
    payload,
    publication_status
) VALUES ($1, $2, $3, $4, $5, 'PENDING')
"#;

const MARK_PUBLISHED_SQL: &str = r#"
UPDATE v2.event_outbox
SET publication_status = 'PUBLISHED',
    published_at = NOW(),
    locked_until = NULL,
    last_error = NULL,
    modified_at = NOW()
WHERE event_id = $1
  AND publication_status <> 'PUBLISHED'
"#;

const RECORD_PUBLISH_ERROR_SQL: &str = r#"
UPDATE v2.event_outbox
SET publication_status = 'PENDING',
    locked_until = NULL,
    last_error = $2,
    retry_count = retry_count + 1,
    modified_at = NOW()
WHERE event_id = $1
  AND publication_status <> 'PUBLISHED'
"#;

#[derive(Clone)]
pub struct OutboxEventPublisher {
    pool: PgPool,
    consultation_topic: String,
    delegate: Arc<dyn EventPublisher>,
}

impl OutboxEventPublisher {
    pub fn new(
        pool: PgPool,
        consultation_topic: String,
        delegate: Arc<dyn EventPublisher>,
    ) -> Self {
        Self {
            pool,
            consultation_topic,
            delegate,
        }
    }

    async fn enqueue<T: serde::Serialize + ?Sized>(
        &self,
        topic: &str,
        event_type: &str,
        aggregate_id: Option<String>,
        event: &T,
    ) -> Result<Uuid, anyhow::Error> {
        let event_id = Uuid::new_v4();
        let payload = serde_json::to_value(event)?;

        sqlx::query(ENQUEUE_DIRECT_PUBLISH_SQL)
            .bind(event_id)
            .bind(topic)
            .bind(event_type)
            .bind(aggregate_id)
            .bind(payload)
            .bind(DIRECT_PUBLISH_LOCK_SECONDS)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to enqueue event outbox row: {e}"))?;

        Ok(event_id)
    }

    async fn mark_published(&self, event_id: Uuid) -> Result<(), anyhow::Error> {
        sqlx::query(MARK_PUBLISHED_SQL)
            .bind(event_id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to mark event outbox row published: {e}"))?;
        Ok(())
    }

    async fn record_publish_error(&self, event_id: Uuid, error: &anyhow::Error) {
        if let Err(db_error) = sqlx::query(RECORD_PUBLISH_ERROR_SQL)
            .bind(event_id)
            .bind(error.to_string())
            .execute(&self.pool)
            .await
        {
            tracing::error!(%db_error, %event_id, "failed to record outbox publish error");
        }
    }
}

pub async fn enqueue_consultation_event_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    consultation_topic: &str,
    event: &ConsultationEvent,
) -> Result<Uuid, anyhow::Error> {
    let event_id = Uuid::new_v4();
    let event_type = consultation_event_type(event);
    let aggregate_id = consultation_event_aggregate_id(event);
    let payload = serde_json::to_value(event)?;

    sqlx::query(ENQUEUE_PENDING_SQL)
        .bind(event_id)
        .bind(consultation_topic)
        .bind(event_type)
        .bind(aggregate_id)
        .bind(payload)
        .execute(&mut **tx)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to enqueue consultation event outbox row: {e}"))?;

    Ok(event_id)
}

#[async_trait::async_trait]
impl EventPublisher for OutboxEventPublisher {
    async fn publish_consultation_event(
        &self,
        event: ConsultationEvent,
    ) -> Result<(), anyhow::Error> {
        let event_type = consultation_event_type(&event);
        let aggregate_id = consultation_event_aggregate_id(&event);
        let event_id = self
            .enqueue(&self.consultation_topic, event_type, aggregate_id, &event)
            .await?;

        match self
            .delegate
            .publish_consultation_event_with_id(event_id, event)
            .await
        {
            Ok(()) => self.mark_published(event_id).await,
            Err(error) => {
                self.record_publish_error(event_id, &error).await;
                Err(error)
            }
        }
    }

    async fn publish_doctor_timeslot_config_changed_event(
        &self,
        event: DoctorTimeslotConfigChangedEvent,
    ) -> Result<(), anyhow::Error> {
        let (topic, event_type, aggregate_id) =
            doctor_timeslot_config_event_outbox_metadata(&event);
        let event_id = self
            .enqueue(topic, event_type, aggregate_id, &event)
            .await?;

        match self
            .delegate
            .publish_doctor_timeslot_config_changed_event(event)
            .await
        {
            Ok(()) => self.mark_published(event_id).await,
            Err(error) => {
                self.record_publish_error(event_id, &error).await;
                Err(error)
            }
        }
    }
}

fn doctor_timeslot_config_event_outbox_metadata(
    event: &DoctorTimeslotConfigChangedEvent,
) -> (&'static str, &'static str, Option<String>) {
    let topic = event.topic();
    let event_type = event.event_type.as_str();
    let aggregate_id = Some(event.doctor.doctor_id.map_or_else(
        || {
            format!(
                "{}:{}",
                event.doctor.doctor_account_id.unwrap_or_default(),
                event.doctor.doctor_profile_id.unwrap_or_default()
            )
        },
        |doctor_id| doctor_id.to_string(),
    ));

    (topic, event_type, aggregate_id)
}

fn consultation_event_type(event: &ConsultationEvent) -> &'static str {
    match event {
        ConsultationEvent::PreSessionMessage(event) => pre_session_message_event_type(event),
        ConsultationEvent::SessionMessage(event) => session_message_event_type(event),
        ConsultationEvent::PostSessionMessage(event) => post_session_message_event_type(event),
    }
}

fn pre_session_message_event_type(event: &PreSessionMessage) -> &'static str {
    match event {
        PreSessionMessage::TimeslotReserved { .. } => "TimeslotReserved",
        PreSessionMessage::ReservationCancelled { .. } => "ReservationCancelled",
        PreSessionMessage::ReservationExpired { .. } => "ReservationExpired",
        PreSessionMessage::ConsultationBooked { .. } => "ConsultationBooked",
        PreSessionMessage::ConsultationCancelled { .. } => "ConsultationCancelled",
    }
}

fn session_message_event_type(event: &SessionMessage) -> &'static str {
    match event {
        SessionMessage::SessionCreated { .. } => "SessionCreated",
        SessionMessage::PatientJoined { .. } => "PatientJoined",
        SessionMessage::DoctorJoined { .. } => "DoctorJoined",
        SessionMessage::AllParticipantJoined { .. } => "AllParticipantJoined",
        SessionMessage::PatientDisconnected { .. } => "PatientDisconnected",
        SessionMessage::DoctorDisconnected { .. } => "DoctorDisconnected",
        SessionMessage::SessionTerminated { .. } => "SessionTerminated",
    }
}

fn post_session_message_event_type(event: &PostSessionMessage) -> &'static str {
    match event {
        PostSessionMessage::ConsultationSummarized { .. } => "ConsultationSummarized",
        PostSessionMessage::FollowUpRequired { .. } => "FollowUpRequired",
        PostSessionMessage::FollowUpRequestExpired { .. } => "FollowUpRequestExpired",
        PostSessionMessage::PatientAcceptedFollowUp { .. } => "PatientAcceptedFollowUp",
        PostSessionMessage::FollowUpCancelled { .. } => "FollowUpCancelled",
    }
}

fn consultation_event_aggregate_id(event: &ConsultationEvent) -> Option<String> {
    Some(match event {
        ConsultationEvent::PreSessionMessage(event) => match event {
            PreSessionMessage::TimeslotReserved { booking_id, .. }
            | PreSessionMessage::ReservationCancelled { booking_id, .. }
            | PreSessionMessage::ReservationExpired { booking_id, .. }
            | PreSessionMessage::ConsultationBooked { booking_id, .. }
            | PreSessionMessage::ConsultationCancelled { booking_id, .. } => booking_id.clone(),
        },
        ConsultationEvent::SessionMessage(event) => match event {
            SessionMessage::SessionCreated { booking_id, .. }
            | SessionMessage::PatientJoined { booking_id, .. }
            | SessionMessage::DoctorJoined { booking_id, .. }
            | SessionMessage::AllParticipantJoined { booking_id, .. }
            | SessionMessage::PatientDisconnected { booking_id, .. }
            | SessionMessage::DoctorDisconnected { booking_id, .. }
            | SessionMessage::SessionTerminated { booking_id, .. } => booking_id.clone(),
        },
        ConsultationEvent::PostSessionMessage(event) => match event {
            PostSessionMessage::ConsultationSummarized { booking_id, .. } => booking_id.clone(),
            PostSessionMessage::FollowUpRequired {
                previous_booking_id,
                ..
            }
            | PostSessionMessage::FollowUpRequestExpired {
                previous_booking_id,
                ..
            }
            | PostSessionMessage::PatientAcceptedFollowUp {
                previous_booking_id,
                ..
            }
            | PostSessionMessage::FollowUpCancelled {
                previous_booking_id,
                ..
            } => previous_booking_id.clone(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::tdh_protocol::common::PartialUserIdentity;
    use crate::consultation_config::model::DoctorConfigIdentity;
    use crate::doctor_timeslot::configuration_events::model::{
        DoctorTimeslotConfigChangeType, TIMESLOT_CONFIGURATION_CHANGED_TOPIC,
    };

    fn normalize_sql(sql: &str) -> String {
        sql.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn enqueue_sql_leases_pending_rows_during_direct_publish() {
        let sql = normalize_sql(ENQUEUE_DIRECT_PUBLISH_SQL);

        assert!(DIRECT_PUBLISH_LOCK_SECONDS > 0);
        assert!(sql.contains("publication_status, locked_until"));
        assert!(sql.contains(
            "VALUES ($1, $2, $3, $4, $5, 'PENDING', NOW() + ($6::integer * INTERVAL '1 second'))"
        ));
    }

    #[test]
    fn transaction_enqueue_sql_leaves_pending_rows_unlocked_for_worker() {
        let sql = normalize_sql(ENQUEUE_PENDING_SQL);

        assert!(sql.contains("INSERT INTO v2.event_outbox"));
        assert!(sql.contains("publication_status"));
        assert!(sql.contains("VALUES ($1, $2, $3, $4, $5, 'PENDING')"));
        assert!(!sql.contains("locked_until"));
    }

    #[test]
    fn mark_published_sql_clears_direct_publish_lock() {
        let sql = normalize_sql(MARK_PUBLISHED_SQL);

        assert!(sql.contains("publication_status = 'PUBLISHED'"));
        assert!(sql.contains("published_at = NOW()"));
        assert!(sql.contains("locked_until = NULL"));
        assert!(sql.contains("last_error = NULL"));
    }

    #[test]
    fn record_publish_error_sql_releases_row_for_retry() {
        let sql = normalize_sql(RECORD_PUBLISH_ERROR_SQL);

        assert!(sql.contains("publication_status = 'PENDING'"));
        assert!(sql.contains("locked_until = NULL"));
        assert!(sql.contains("last_error = $2"));
        assert!(sql.contains("retry_count = retry_count + 1"));
        assert!(sql.contains("publication_status <> 'PUBLISHED'"));
    }

    #[test]
    fn derives_event_type_and_aggregate_from_consultation_event() {
        let event = ConsultationEvent::SessionMessage(SessionMessage::PatientJoined {
            booking_id: "BK1".to_string(),
            patient_identity: PartialUserIdentity {
                account_id: 1,
                user_profile_id: 2,
                tenant_id: 1,
                oidc_user_id: None,
            },
            doctor_id: 3,
            joined_at: 4,
        });

        assert_eq!(consultation_event_type(&event), "PatientJoined");
        assert_eq!(
            consultation_event_aggregate_id(&event).as_deref(),
            Some("BK1")
        );
    }

    #[test]
    fn derives_doctor_timeslot_config_event_outbox_metadata() {
        let doctor_id = Uuid::from_u128(0x1234567890abcdef1234567890abcdef);
        let event = DoctorTimeslotConfigChangedEvent::new(
            DoctorConfigIdentity {
                doctor_id,
                doctor_account_id: 123,
                doctor_profile_id: 456,
            },
            DoctorTimeslotConfigChangeType::TimeslotConfiguration,
            Some(true),
        );

        let (topic, event_type, aggregate_id) =
            doctor_timeslot_config_event_outbox_metadata(&event);

        assert_eq!(topic, TIMESLOT_CONFIGURATION_CHANGED_TOPIC);
        assert_eq!(event_type, "DoctorTimeslotConfigurationChanged");
        assert_eq!(
            aggregate_id.as_deref(),
            Some(doctor_id.to_string().as_str())
        );
    }

    #[test]
    fn derives_doctor_timeslot_config_event_fallback_aggregate_without_doctor_uuid() {
        let mut event = DoctorTimeslotConfigChangedEvent::new(
            DoctorConfigIdentity {
                doctor_id: Uuid::from_u128(0x1234567890abcdef1234567890abcdef),
                doctor_account_id: 123,
                doctor_profile_id: 456,
            },
            DoctorTimeslotConfigChangeType::TimeslotConfiguration,
            Some(true),
        );
        event.doctor.doctor_id = None;

        let (_topic, _event_type, aggregate_id) =
            doctor_timeslot_config_event_outbox_metadata(&event);

        assert_eq!(aggregate_id.as_deref(), Some("123:456"));
    }
}
