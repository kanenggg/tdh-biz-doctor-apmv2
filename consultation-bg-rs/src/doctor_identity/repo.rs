use async_trait::async_trait;
use uuid::Uuid;

use super::model::DoctorServiceConfig;

#[derive(Debug, Clone)]
pub struct DoctorProfileProjection {
    pub event_id: String,
    pub doctor_id: Uuid,
    pub doctor_account_id: i64,
    pub doctor_profile_id: i64,
    pub is_active: bool,
    pub profile_version: Option<i64>,
    pub source_occurred_at: i64,
    pub consultation_config: DoctorServiceConfig,
}

#[derive(Debug, Clone)]
pub struct DoctorDeactivationProjection {
    pub event_id: String,
    pub doctor_id: Uuid,
    pub doctor_account_id: i64,
    pub doctor_profile_id: i64,
    pub profile_version: Option<i64>,
    pub source_occurred_at: i64,
}

#[derive(Debug, thiserror::Error)]
#[error(
    "doctor profile projection contract conflict: ordering coordinate {coordinate} has eventId {existing_event_id}, not {event_id}"
)]
pub struct ProjectionContractConflict {
    pub coordinate: String,
    pub existing_event_id: String,
    pub event_id: String,
}

#[async_trait]
pub trait DoctorIdentityRepo: Send + Sync {
    async fn apply_projection(
        &self,
        projection: DoctorProfileProjection,
    ) -> Result<(), anyhow::Error>;
    async fn deactivate(
        &self,
        projection: DoctorDeactivationProjection,
    ) -> Result<(), anyhow::Error>;
}

#[derive(Clone)]
pub struct DoctorIdentityPsql {
    pool: sqlx::PgPool,
}

impl DoctorIdentityPsql {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }
}

#[derive(Debug, Clone)]
struct ExistingOrdering {
    profile_version: Option<i64>,
    source_occurred_at: Option<i64>,
    source_event_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OrderingAction {
    Apply,
    Noop,
}

fn ordering_action(
    existing: Option<ExistingOrdering>,
    profile_version: Option<i64>,
    source_occurred_at: i64,
    event_id: &str,
) -> Result<OrderingAction, ProjectionContractConflict> {
    let Some(existing) = existing else {
        return Ok(OrderingAction::Apply);
    };

    match (profile_version, existing.profile_version) {
        (Some(incoming), Some(current)) if incoming > current => Ok(OrderingAction::Apply),
        (Some(incoming), Some(current)) if incoming < current => Ok(OrderingAction::Noop),
        (Some(incoming), Some(_)) => same_coordinate_action(
            format!("profileVersion:{incoming}"),
            existing.source_event_id,
            event_id,
        ),
        // Versioned events always supersede the committed unversioned stream.
        (Some(_), None) => Ok(OrderingAction::Apply),
        // A committed V1 event is never allowed to overwrite versioned state.
        (None, Some(_)) => Ok(OrderingAction::Noop),
        (None, None) => match existing.source_occurred_at {
            Some(current) if source_occurred_at > current => Ok(OrderingAction::Apply),
            Some(current) if source_occurred_at < current => Ok(OrderingAction::Noop),
            Some(_) => same_coordinate_action(
                format!("occurredAt:{source_occurred_at}"),
                existing.source_event_id,
                event_id,
            ),
            // Rows from before source_occurred_at was introduced have no
            // reliable V1 watermark, so a complete committed snapshot repairs them.
            None => Ok(OrderingAction::Apply),
        },
    }
}

fn same_coordinate_action(
    coordinate: String,
    existing_event_id: Option<String>,
    event_id: &str,
) -> Result<OrderingAction, ProjectionContractConflict> {
    if existing_event_id.as_deref() == Some(event_id) {
        Ok(OrderingAction::Noop)
    } else {
        Err(ProjectionContractConflict {
            coordinate,
            existing_event_id: existing_event_id.unwrap_or_default(),
            event_id: event_id.to_string(),
        })
    }
}

async fn locked_ordering(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    doctor_id: Uuid,
) -> Result<Option<ExistingOrdering>, anyhow::Error> {
    Ok(sqlx::query_as::<_, (Option<i64>, Option<i64>, Option<String>)>(
        "SELECT profile_version, source_occurred_at, source_event_id FROM v2.doctor_identity WHERE doctor_id = $1 FOR UPDATE",
    )
    .bind(doctor_id)
    .fetch_optional(&mut **tx)
    .await?
    .map(|(profile_version, source_occurred_at, source_event_id)| ExistingOrdering {
        profile_version,
        source_occurred_at,
        source_event_id,
    }))
}

async fn lock_projection_key(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    doctor_id: Uuid,
) -> Result<(), anyhow::Error> {
    // Row locks cannot serialize competing first writes. This transaction-scoped
    // advisory lock covers both identity and service-config rows for one doctor.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1::text, 0))")
        .bind(doctor_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn upsert_identity(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    doctor_id: Uuid,
    doctor_account_id: i64,
    doctor_profile_id: i64,
    is_active: bool,
    event_id: &str,
    profile_version: Option<i64>,
    source_occurred_at: i64,
) -> Result<(), anyhow::Error> {
    sqlx::query(
        r#"
        INSERT INTO v2.doctor_identity
            (doctor_id, doctor_account_id, doctor_profile_id, is_active, source_event_id, profile_version, source_occurred_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, now())
        ON CONFLICT (doctor_id) DO UPDATE SET
            doctor_account_id = EXCLUDED.doctor_account_id,
            doctor_profile_id = EXCLUDED.doctor_profile_id,
            is_active = EXCLUDED.is_active,
            source_event_id = EXCLUDED.source_event_id,
            profile_version = EXCLUDED.profile_version,
            source_occurred_at = EXCLUDED.source_occurred_at,
            updated_at = now()
        "#,
    )
    .bind(doctor_id)
    .bind(doctor_account_id)
    .bind(doctor_profile_id)
    .bind(is_active)
    .bind(event_id)
    .bind(profile_version)
    .bind(source_occurred_at)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[async_trait]
impl DoctorIdentityRepo for DoctorIdentityPsql {
    async fn apply_projection(
        &self,
        projection: DoctorProfileProjection,
    ) -> Result<(), anyhow::Error> {
        let mut tx = self.pool.begin().await?;
        lock_projection_key(&mut tx, projection.doctor_id).await?;
        match ordering_action(
            locked_ordering(&mut tx, projection.doctor_id).await?,
            projection.profile_version,
            projection.source_occurred_at,
            &projection.event_id,
        ) {
            Ok(OrderingAction::Noop) => return Ok(()),
            Err(conflict) => return Err(conflict.into()),
            Ok(OrderingAction::Apply) => {}
        }

        upsert_identity(
            &mut tx,
            projection.doctor_id,
            projection.doctor_account_id,
            projection.doctor_profile_id,
            projection.is_active,
            &projection.event_id,
            projection.profile_version,
            projection.source_occurred_at,
        )
        .await?;

        let config = projection.consultation_config;
        let channels = config
            .channels
            .into_iter()
            .map(|channel| channel.as_str().to_string())
            .collect::<Vec<_>>();
        let languages = config
            .languages
            .into_iter()
            .map(|language| language.as_str().to_string())
            .collect::<Vec<_>>();
        sqlx::query(
            r#"
            INSERT INTO v2.doctor_service_config_projection
                (doctor_id, channels, languages, duration_minutes, doctor_fee_amount, doctor_fee_currency, profile_version, source_occurred_at, source_event_id, updated_at)
            VALUES ($1, $2, $3, $4, $5::numeric, $6, $7, $8, $9, now())
            ON CONFLICT (doctor_id) DO UPDATE SET
                channels = EXCLUDED.channels,
                languages = EXCLUDED.languages,
                duration_minutes = EXCLUDED.duration_minutes,
                doctor_fee_amount = EXCLUDED.doctor_fee_amount,
                doctor_fee_currency = EXCLUDED.doctor_fee_currency,
                profile_version = EXCLUDED.profile_version,
                source_occurred_at = EXCLUDED.source_occurred_at,
                source_event_id = EXCLUDED.source_event_id,
                updated_at = now()
            "#,
        )
        .bind(projection.doctor_id)
        .bind(channels)
        .bind(languages)
        .bind(config.duration_minutes)
        .bind(config.fee_amount.to_string())
        .bind(config.currency)
        .bind(projection.profile_version)
        .bind(projection.source_occurred_at)
        .bind(&projection.event_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

    async fn deactivate(
        &self,
        projection: DoctorDeactivationProjection,
    ) -> Result<(), anyhow::Error> {
        let mut tx = self.pool.begin().await?;
        lock_projection_key(&mut tx, projection.doctor_id).await?;
        match ordering_action(
            locked_ordering(&mut tx, projection.doctor_id).await?,
            projection.profile_version,
            projection.source_occurred_at,
            &projection.event_id,
        ) {
            Ok(OrderingAction::Noop) => return Ok(()),
            Err(conflict) => return Err(conflict.into()),
            Ok(OrderingAction::Apply) => {}
        }
        upsert_identity(
            &mut tx,
            projection.doctor_id,
            projection.doctor_account_id,
            projection.doctor_profile_id,
            false,
            &projection.event_id,
            projection.profile_version,
            projection.source_occurred_at,
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fs2::FileExt;
    use sqlx::postgres::PgPoolOptions;
    use std::{fs::OpenOptions, sync::Arc};
    use testcontainers::{
        GenericImage, ImageExt,
        core::{IntoContainerPort, WaitFor},
        runners::AsyncRunner,
    };

    use crate::doctor_identity::{
        model::DoctorProfileEvent,
        service::{DoctorIdentityError, DoctorIdentityService},
    };

    fn existing(version: Option<i64>, occurred_at: i64, event_id: &str) -> ExistingOrdering {
        ExistingOrdering {
            profile_version: version,
            source_occurred_at: Some(occurred_at),
            source_event_id: Some(event_id.to_string()),
        }
    }

    #[test]
    fn committed_events_use_occurred_at_for_duplicate_stale_and_conflict_decisions() {
        let current = Some(existing(None, 100, "evt-100"));
        assert_eq!(
            ordering_action(current.clone(), None, 100, "evt-100").unwrap(),
            OrderingAction::Noop
        );
        assert_eq!(
            ordering_action(current.clone(), None, 99, "evt-99").unwrap(),
            OrderingAction::Noop
        );
        assert!(ordering_action(current, None, 100, "other-event").is_err());
    }

    #[test]
    fn versioned_watermark_supersedes_and_is_never_overwritten_by_committed_v1() {
        let current = Some(existing(Some(7), 100, "evt-v2"));
        assert_eq!(
            ordering_action(current.clone(), None, 101, "evt-v1-newer").unwrap(),
            OrderingAction::Noop
        );
        assert_eq!(
            ordering_action(current.clone(), Some(6), 999, "evt-v2-old").unwrap(),
            OrderingAction::Noop
        );
        assert_eq!(
            ordering_action(current, Some(8), 1, "evt-v2-new").unwrap(),
            OrderingAction::Apply
        );
    }

    async fn projection_test_database() -> (
        sqlx::PgPool,
        testcontainers::ContainerAsync<GenericImage>,
        std::fs::File,
    ) {
        let port_lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open("/tmp/hermes-apmv2-doctor-identity-postgres-55440.lock")
            .expect("PostgreSQL port lock file must open");
        FileExt::lock_exclusive(&port_lock).expect("PostgreSQL fixed port lock must be acquired");

        let container = GenericImage::new("postgres", "14-alpine")
            .with_wait_for(WaitFor::message_on_stderr(
                "database system is ready to accept connections",
            ))
            .with_mapped_port(55440, 5432.tcp())
            .with_env_var("POSTGRES_DB", "postgres")
            .with_env_var("POSTGRES_USER", "postgres")
            .with_env_var("POSTGRES_PASSWORD", "postgres")
            .start()
            .await
            .expect("PostgreSQL Testcontainer must start");
        let host = container
            .get_host()
            .await
            .expect("PostgreSQL Testcontainer host must be available");
        let database_url = format!("postgresql://postgres:postgres@{host}:55440/postgres");
        let pool = loop {
            match PgPoolOptions::new()
                .max_connections(5)
                .connect(&database_url)
                .await
            {
                Ok(pool) => break pool,
                Err(_) => tokio::time::sleep(tokio::time::Duration::from_millis(250)).await,
            }
        };
        for migration in [
            include_str!("../../../db/biz_apm/migrations/20260220000001__create_schema.sql"),
            include_str!(
                "../../../db/biz_apm/migrations/20260706000001__doctor_consultation_config.sql"
            ),
            include_str!(
                "../../../db/biz_apm/migrations/20260713000001__doctor_service_config_projection.sql"
            ),
            include_str!(
                "../../../db/biz_apm/migrations/20260713000002__doctor_service_config_projection_source_ordering.sql"
            ),
        ] {
            sqlx::raw_sql(migration)
                .execute(&pool)
                .await
                .expect("projection migration must apply to a fresh PostgreSQL database");
        }
        (pool, container, port_lock)
    }

    fn committed_event(
        event_id: &str,
        occurred_at: i64,
        fee: i32,
        profile_version: Option<i64>,
    ) -> DoctorProfileEvent {
        let mut payload: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/doctor_profile_approved_origin_main.json"
        ))
        .unwrap();
        payload["eventId"] = serde_json::json!(event_id);
        payload["occurredAt"] = serde_json::json!(occurred_at);
        payload["doctorFee"] = serde_json::json!(fee);
        if let Some(profile_version) = profile_version {
            payload["schemaVersion"] = serde_json::json!(2);
            payload["profileVersion"] = serde_json::json!(profile_version);
            payload["consultationConfig"] = serde_json::json!({
                "channels": ["voice", "chat"], "languages": ["th", "en"],
                "durationMinutes": 15, "feeAmount": format!("{fee}.00"), "currency": "THB"
            });
        }
        serde_json::from_value(payload).unwrap()
    }

    #[tokio::test]
    async fn postgres_projection_is_atomic_and_enforces_committed_event_ordering() {
        let (pool, _container, _port_lock) = projection_test_database().await;
        let service =
            DoctorIdentityService::new(Arc::new(DoctorIdentityPsql::new(pool.clone())), vec![2]);

        service
            .handle_event(committed_event("evt-100", 100, 650, None))
            .await
            .unwrap();
        let identity: (i64, i64, bool, Option<i64>, Option<i64>, String) = sqlx::query_as(
            "SELECT doctor_account_id, doctor_profile_id, is_active, profile_version, source_occurred_at, source_event_id FROM v2.doctor_identity",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let config: (Vec<String>, Vec<String>, i32, String, String, Option<i64>, Option<i64>, i64, String) = sqlx::query_as(
            "SELECT channels, languages, duration_minutes, doctor_fee_amount::text, doctor_fee_currency, profile_version, source_occurred_at, effective_source_version, source_event_id FROM v2.doctor_service_config_projection",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            identity,
            (2443, 8891, true, None, Some(100), "evt-100".to_string())
        );
        assert_eq!(
            config,
            (
                vec!["voice".to_string(), "chat".to_string()],
                vec!["th".to_string(), "en".to_string()],
                15,
                "650.00".to_string(),
                "THB".to_string(),
                None,
                Some(100),
                100,
                "evt-100".to_string(),
            )
        );

        service
            .handle_event(committed_event("evt-100", 100, 650, None))
            .await
            .unwrap();
        service
            .handle_event(committed_event("evt-99", 99, 999, None))
            .await
            .unwrap();
        assert!(matches!(
            service
                .handle_event(committed_event("other-100", 100, 999, None))
                .await,
            Err(DoctorIdentityError::ContractConflict)
        ));

        service
            .handle_event(committed_event("evt-v2-7", 200, 700, Some(7)))
            .await
            .unwrap();
        service
            .handle_event(committed_event("evt-v1-after-v2", 300, 999, None))
            .await
            .unwrap();
        let final_config: (String, Option<i64>, Option<i64>, i64, String) = sqlx::query_as(
            "SELECT doctor_fee_amount::text, profile_version, source_occurred_at, effective_source_version, source_event_id FROM v2.doctor_service_config_projection",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            final_config,
            (
                "700.00".to_string(),
                Some(7),
                Some(200),
                7,
                "evt-v2-7".to_string()
            )
        );
    }
}
