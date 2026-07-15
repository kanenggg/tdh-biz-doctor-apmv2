use async_trait::async_trait;
use sqlx::PgPool;

use super::model::{ConsultationAvailability, DoctorConfigIdentity, ScheduleAvailableConfig};
use crate::doctor_timeslot::configuration_events::model::DoctorTimeslotConfigChangedEvent;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ConsultationConfigRepoError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("invalid schedule config JSON: {0}")]
    InvalidScheduleConfig(#[from] serde_json::Error),
}

#[async_trait]
pub trait ConsultationConfigRepo: Send + Sync {
    async fn resolve_current_doctor_identity(
        &self,
        doctor_account_id: i64,
        doctor_profile_id: i64,
    ) -> Result<Option<DoctorConfigIdentity>, ConsultationConfigRepoError>;

    async fn get_schedule_config(
        &self,
        identity: DoctorConfigIdentity,
    ) -> Result<Option<ScheduleAvailableConfig>, ConsultationConfigRepoError>;

    async fn save_schedule_config(
        &self,
        identity: DoctorConfigIdentity,
        config: &ScheduleAvailableConfig,
    ) -> Result<ConsultationAvailability, ConsultationConfigRepoError>;

    async fn save_schedule_config_and_enqueue(
        &self,
        identity: DoctorConfigIdentity,
        config: &ScheduleAvailableConfig,
        event: &DoctorTimeslotConfigChangedEvent,
    ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
        self.save_schedule_config(identity, config).await
    }

    async fn set_schedule_availability(
        &self,
        identity: DoctorConfigIdentity,
        available: bool,
    ) -> Result<ConsultationAvailability, ConsultationConfigRepoError>;

    async fn set_schedule_availability_and_enqueue(
        &self,
        identity: DoctorConfigIdentity,
        available: bool,
        event: &DoctorTimeslotConfigChangedEvent,
    ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
        self.set_schedule_availability(identity, available).await
    }

    async fn set_instant_availability(
        &self,
        identity: DoctorConfigIdentity,
        available: bool,
    ) -> Result<ConsultationAvailability, ConsultationConfigRepoError>;

    async fn set_instant_availability_and_enqueue(
        &self,
        identity: DoctorConfigIdentity,
        available: bool,
        event: &DoctorTimeslotConfigChangedEvent,
    ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
        self.set_instant_availability(identity, available).await
    }

    async fn get_availability(
        &self,
        identity: DoctorConfigIdentity,
    ) -> Result<ConsultationAvailability, ConsultationConfigRepoError>;
}

#[derive(Debug, Clone)]
pub struct ConsultationConfigRepoPsql {
    pool: PgPool,
}

impl ConsultationConfigRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

async fn enqueue_event_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event: &DoctorTimeslotConfigChangedEvent,
) -> Result<(), ConsultationConfigRepoError> {
    sqlx::query(
        "INSERT INTO v2.event_outbox (event_id, topic, event_type, aggregate_id, payload, publication_status) VALUES ($1, $2, $3, $4, $5, 'PENDING')",
    )
    .bind(Uuid::parse_str(&event.event_id).map_err(|error| ConsultationConfigRepoError::Database(sqlx::Error::Protocol(error.to_string())))?)
    .bind(event.topic())
    .bind(event.event_type.as_str())
    .bind(event.doctor.doctor_id.map(|id| id.to_string()))
    .bind(serde_json::to_value(event)?)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

const RESOLVE_CURRENT_DOCTOR_IDENTITY_SQL: &str = r#"
            SELECT doctor_id, doctor_account_id, doctor_profile_id
            FROM v2.doctor_info_projection
            WHERE doctor_account_id = $1
              AND doctor_profile_id = $2
              AND is_active = true
            "#;

#[derive(sqlx::FromRow)]
struct ConfigRow {
    schedule_config: serde_json::Value,
}

#[derive(sqlx::FromRow)]
struct AvailabilityRow {
    schedule_available: bool,
    instant_available: bool,
}

#[async_trait]
impl ConsultationConfigRepo for ConsultationConfigRepoPsql {
    async fn resolve_current_doctor_identity(
        &self,
        doctor_account_id: i64,
        doctor_profile_id: i64,
    ) -> Result<Option<DoctorConfigIdentity>, ConsultationConfigRepoError> {
        let row = sqlx::query_as::<_, (uuid::Uuid, i64, i64)>(RESOLVE_CURRENT_DOCTOR_IDENTITY_SQL)
            .bind(doctor_account_id)
            .bind(doctor_profile_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(
            |(doctor_id, doctor_account_id, doctor_profile_id)| DoctorConfigIdentity {
                doctor_id,
                doctor_account_id,
                doctor_profile_id,
            },
        ))
    }

    async fn get_schedule_config(
        &self,
        identity: DoctorConfigIdentity,
    ) -> Result<Option<ScheduleAvailableConfig>, ConsultationConfigRepoError> {
        let row = sqlx::query_as::<_, ConfigRow>(
            r#"
            SELECT schedule_config
            FROM v2.doctor_consultation_config
            WHERE doctor_id = $1
            "#,
        )
        .bind(identity.doctor_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| serde_json::from_value(row.schedule_config))
            .transpose()
            .map_err(ConsultationConfigRepoError::from)
    }

    async fn save_schedule_config(
        &self,
        identity: DoctorConfigIdentity,
        config: &ScheduleAvailableConfig,
    ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
        let config_value = serde_json::to_value(config)?;
        let row = sqlx::query_as::<_, AvailabilityRow>(
            r#"
            INSERT INTO v2.doctor_consultation_config (
                doctor_id,
                schedule_config,
                updated_at
            ) VALUES ($1, $2, now())
            ON CONFLICT (doctor_id) DO UPDATE SET
                schedule_config = EXCLUDED.schedule_config,
                updated_at = now()
            RETURNING schedule_available, instant_available
            "#,
        )
        .bind(identity.doctor_id)
        .bind(config_value)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.into())
    }

    async fn save_schedule_config_and_enqueue(
        &self,
        identity: DoctorConfigIdentity,
        config: &ScheduleAvailableConfig,
        event: &DoctorTimeslotConfigChangedEvent,
    ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_as::<_, AvailabilityRow>(
            "INSERT INTO v2.doctor_consultation_config (doctor_id, schedule_config, updated_at) VALUES ($1, $2, now()) ON CONFLICT (doctor_id) DO UPDATE SET schedule_config = EXCLUDED.schedule_config, updated_at = now() RETURNING schedule_available, instant_available",
        )
        .bind(identity.doctor_id)
        .bind(serde_json::to_value(config)?)
        .fetch_one(&mut *tx)
        .await?;
        enqueue_event_in_tx(&mut tx, event).await?;
        tx.commit().await?;
        Ok(row.into())
    }

    async fn set_schedule_availability(
        &self,
        identity: DoctorConfigIdentity,
        available: bool,
    ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
        let row = sqlx::query_as::<_, AvailabilityRow>(
            r#"
            INSERT INTO v2.doctor_consultation_config (
                doctor_id,
                schedule_available,
                updated_at
            ) VALUES ($1, $2, now())
            ON CONFLICT (doctor_id) DO UPDATE SET
                schedule_available = EXCLUDED.schedule_available,
                updated_at = now()
            RETURNING schedule_available, instant_available
            "#,
        )
        .bind(identity.doctor_id)
        .bind(available)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.into())
    }

    async fn set_schedule_availability_and_enqueue(
        &self,
        identity: DoctorConfigIdentity,
        available: bool,
        event: &DoctorTimeslotConfigChangedEvent,
    ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_as::<_, AvailabilityRow>(
            "INSERT INTO v2.doctor_consultation_config (doctor_id, schedule_available, updated_at) VALUES ($1, $2, now()) ON CONFLICT (doctor_id) DO UPDATE SET schedule_available = EXCLUDED.schedule_available, updated_at = now() RETURNING schedule_available, instant_available",
        )
        .bind(identity.doctor_id)
        .bind(available)
        .fetch_one(&mut *tx)
        .await?;
        enqueue_event_in_tx(&mut tx, event).await?;
        tx.commit().await?;
        Ok(row.into())
    }

    async fn set_instant_availability(
        &self,
        identity: DoctorConfigIdentity,
        available: bool,
    ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
        let row = sqlx::query_as::<_, AvailabilityRow>(
            r#"
            INSERT INTO v2.doctor_consultation_config (
                doctor_id,
                instant_available,
                updated_at
            ) VALUES ($1, $2, now())
            ON CONFLICT (doctor_id) DO UPDATE SET
                instant_available = EXCLUDED.instant_available,
                updated_at = now()
            RETURNING schedule_available, instant_available
            "#,
        )
        .bind(identity.doctor_id)
        .bind(available)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.into())
    }

    async fn set_instant_availability_and_enqueue(
        &self,
        identity: DoctorConfigIdentity,
        available: bool,
        event: &DoctorTimeslotConfigChangedEvent,
    ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query_as::<_, AvailabilityRow>(
            "INSERT INTO v2.doctor_consultation_config (doctor_id, instant_available, updated_at) VALUES ($1, $2, now()) ON CONFLICT (doctor_id) DO UPDATE SET instant_available = EXCLUDED.instant_available, updated_at = now() RETURNING schedule_available, instant_available",
        )
        .bind(identity.doctor_id)
        .bind(available)
        .fetch_one(&mut *tx)
        .await?;
        enqueue_event_in_tx(&mut tx, event).await?;
        tx.commit().await?;
        Ok(row.into())
    }

    async fn get_availability(
        &self,
        identity: DoctorConfigIdentity,
    ) -> Result<ConsultationAvailability, ConsultationConfigRepoError> {
        let row = sqlx::query_as::<_, AvailabilityRow>(
            r#"
            SELECT schedule_available, instant_available
            FROM v2.doctor_consultation_config
            WHERE doctor_id = $1
            "#,
        )
        .bind(identity.doctor_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Into::into).unwrap_or_default())
    }
}

impl From<AvailabilityRow> for ConsultationAvailability {
    fn from(row: AvailabilityRow) -> Self {
        Self {
            schedule_available: row.schedule_available,
            instant_available: row.instant_available,
        }
    }
}

impl Default for ConsultationAvailability {
    fn default() -> Self {
        Self {
            schedule_available: false,
            instant_available: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consultation_config_doctor_identity_lookup_sql_uses_active_projection() {
        assert!(RESOLVE_CURRENT_DOCTOR_IDENTITY_SQL.contains("FROM v2.doctor_info_projection"));
        assert!(RESOLVE_CURRENT_DOCTOR_IDENTITY_SQL.contains("doctor_account_id = $1"));
        assert!(RESOLVE_CURRENT_DOCTOR_IDENTITY_SQL.contains("doctor_profile_id = $2"));
        assert!(RESOLVE_CURRENT_DOCTOR_IDENTITY_SQL.contains("is_active = true"));
    }

    #[test]
    fn consultation_config_doctor_identity_lookup_sql_selects_current_identity_fields() {
        assert!(
            RESOLVE_CURRENT_DOCTOR_IDENTITY_SQL
                .contains("SELECT doctor_id, doctor_account_id, doctor_profile_id")
        );
    }
}
