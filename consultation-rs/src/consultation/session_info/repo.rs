use crate::common::tdh_protocol::iam::user_identity::{AccountType, UserIdentity};
use sqlx::PgPool;
use tracing::instrument;

use crate::{
    consultation::common::{DbConsultationSession, DbSessionDetails, SessionDetails},
    repo::provider_session_info::SessionData,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionParticipantRole {
    Patient,
    Doctor,
}

impl From<AccountType> for SessionParticipantRole {
    fn from(value: AccountType) -> Self {
        match value {
            AccountType::Patient => Self::Patient,
            AccountType::Doctor => Self::Doctor,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipantJoinRecord {
    pub participant_joined_first_time: bool,
    pub all_participants_joined_first_time: bool,
    pub patient_joined_at: Option<i64>,
    pub doctor_joined_at: Option<i64>,
}

const MARK_PATIENT_JOINED_SQL: &str = r#"
WITH locked AS (
    SELECT patient_joined_at, doctor_joined_at
    FROM v2.session_info
    WHERE appointment_id = $1
    FOR UPDATE
), updated AS (
    UPDATE v2.session_info
    SET patient_joined_at = COALESCE(v2.session_info.patient_joined_at, to_timestamp($2)),
        session_status = CASE
            WHEN v2.session_info.doctor_joined_at IS NOT NULL THEN 'ALL_PARTICIPANTS_JOINED'::v2.session_info_status_enum
            ELSE 'PATIENT_JOINED'::v2.session_info_status_enum
        END,
        modified_at = NOW()
    WHERE appointment_id = $1
    RETURNING patient_joined_at, doctor_joined_at
)
SELECT
    COALESCE((SELECT patient_joined_at IS NULL FROM locked), false) AS participant_joined_first_time,
    COALESCE((SELECT patient_joined_at IS NULL AND doctor_joined_at IS NOT NULL FROM locked), false) AS all_participants_joined_first_time,
    (SELECT EXTRACT(EPOCH FROM patient_joined_at)::bigint FROM updated) AS patient_joined_at,
    (SELECT EXTRACT(EPOCH FROM doctor_joined_at)::bigint FROM updated) AS doctor_joined_at
"#;

const MARK_DOCTOR_JOINED_SQL: &str = r#"
WITH locked AS (
    SELECT patient_joined_at, doctor_joined_at
    FROM v2.session_info
    WHERE appointment_id = $1
    FOR UPDATE
), updated AS (
    UPDATE v2.session_info
    SET doctor_joined_at = COALESCE(v2.session_info.doctor_joined_at, to_timestamp($2)),
        session_status = CASE
            WHEN v2.session_info.patient_joined_at IS NOT NULL THEN 'ALL_PARTICIPANTS_JOINED'::v2.session_info_status_enum
            ELSE 'DOCTOR_JOINED'::v2.session_info_status_enum
        END,
        modified_at = NOW()
    WHERE appointment_id = $1
    RETURNING patient_joined_at, doctor_joined_at
)
SELECT
    COALESCE((SELECT doctor_joined_at IS NULL FROM locked), false) AS participant_joined_first_time,
    COALESCE((SELECT doctor_joined_at IS NULL AND patient_joined_at IS NOT NULL FROM locked), false) AS all_participants_joined_first_time,
    (SELECT EXTRACT(EPOCH FROM patient_joined_at)::bigint FROM updated) AS patient_joined_at,
    (SELECT EXTRACT(EPOCH FROM doctor_joined_at)::bigint FROM updated) AS doctor_joined_at
"#;

#[async_trait::async_trait]
pub trait SessionManagementRepo: Send + Sync {
    async fn get_appointment_session(
        &self,
        user_id: &UserIdentity,
        appointment_id: &str,
    ) -> Result<Option<DbConsultationSession>, anyhow::Error>;

    async fn init_session_data(
        &self,
        appointment_id: &str,
        sesion_data: SessionData,
    ) -> Result<bool, anyhow::Error>;

    async fn get_session_details(
        &self,
        appointment_id: &str,
    ) -> Result<Option<SessionDetails>, anyhow::Error>;

    async fn mark_participant_joined(
        &self,
        appointment_id: &str,
        participant: SessionParticipantRole,
        joined_at: i64,
    ) -> Result<ParticipantJoinRecord, anyhow::Error>;
}

pub struct GetOrCreateSessionRepoPsql {
    pool: PgPool,
}

impl GetOrCreateSessionRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl SessionManagementRepo for GetOrCreateSessionRepoPsql {
    #[instrument(skip(self, user_id))]
    async fn get_appointment_session(
        &self,
        user_id: &UserIdentity,
        appointment_id: &str,
    ) -> Result<Option<DbConsultationSession>, anyhow::Error> {
        let db_session = sqlx::query_as::<_, DbConsultationSession>(
            r#"
            SELECT * FROM v2.get_consultation_session($1, $2)
            "#,
        )
        .bind(appointment_id)
        .bind(user_id.user_profile_id as i32)
        .fetch_optional(&self.pool)
        .await
        .inspect_err(|e| tracing::error!("{e:?}"))
        .map_err(|e| anyhow::anyhow!("Failed to get appointment session: {}", e))?;

        Ok(db_session.map(|db| db.into()))
    }

    async fn init_session_data(
        &self,
        appointment_id: &str,
        sesion_data: SessionData,
    ) -> Result<bool, anyhow::Error> {
        let session_data_json = serde_json::to_value(&sesion_data)?;

        sqlx::query_scalar::<_, bool>(
            r#"
            SELECT v2.upsert_session_info($1, $2)
            "#,
        )
        .bind(appointment_id)
        .bind(&session_data_json)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to init session data: {}", e))
    }

    async fn get_session_details(
        &self,
        appointment_id: &str,
    ) -> Result<Option<SessionDetails>, anyhow::Error> {
        sqlx::query_as::<_, DbSessionDetails>(
            r#"
            SELECT * FROM v2.get_session_details($1)
            "#,
        )
        .bind(appointment_id)
        .fetch_optional(&self.pool)
        .await
        .map(|opt| opt.map(|db| db.into()))
        .map_err(|e| anyhow::anyhow!("Failed to get session details: {}", e))
    }

    async fn mark_participant_joined(
        &self,
        appointment_id: &str,
        participant: SessionParticipantRole,
        joined_at: i64,
    ) -> Result<ParticipantJoinRecord, anyhow::Error> {
        let sql = match participant {
            SessionParticipantRole::Patient => MARK_PATIENT_JOINED_SQL,
            SessionParticipantRole::Doctor => MARK_DOCTOR_JOINED_SQL,
        };

        let (
            participant_joined_first_time,
            all_participants_joined_first_time,
            patient_joined_at,
            doctor_joined_at,
        ) = sqlx::query_as::<_, (bool, bool, Option<i64>, Option<i64>)>(sql)
            .bind(appointment_id)
            .bind(joined_at as f64)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to mark participant joined: {}", e))?;

        Ok(ParticipantJoinRecord {
            participant_joined_first_time,
            all_participants_joined_first_time,
            patient_joined_at,
            doctor_joined_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_patient_joined_sql_is_idempotent() {
        assert!(MARK_PATIENT_JOINED_SQL.contains("COALESCE(v2.session_info.patient_joined_at"));
        assert!(MARK_PATIENT_JOINED_SQL.contains("ALL_PARTICIPANTS_JOINED"));
    }

    #[test]
    fn mark_doctor_joined_sql_is_idempotent() {
        assert!(MARK_DOCTOR_JOINED_SQL.contains("COALESCE(v2.session_info.doctor_joined_at"));
        assert!(MARK_DOCTOR_JOINED_SQL.contains("ALL_PARTICIPANTS_JOINED"));
    }

    #[test]
    fn init_session_data_reads_atomic_created_flag_from_upsert_function() {
        let migration = include_str!(
            "../../../../db/biz_apm/migrations/20260709000001__session_idempotency_guards.sql"
        );

        assert!(migration.contains("RETURNS boolean"));
        assert!(migration.contains("ON CONFLICT (appointment_id) DO NOTHING"));
        assert!(migration.contains("RETURN COALESCE(v_inserted, false)"));
        assert!(!migration.contains("DO UPDATE SET\n        session_data = EXCLUDED.session_data"));
    }
}
