use sqlx::PgPool;

use crate::consultation::common::{DbSessionDetails, SessionDetails};

const COMPLETE_CONSULTATION_STATUS_TO_DONE_SQL: &str = r#"
SELECT v2.end_active_session($1, $2)
"#;

#[async_trait::async_trait]
pub trait EndSessionRepo: Send + Sync {
    async fn complete_consultation_status_to_done(
        &self,
        appointment_id: &str,
        doctor_profile_id: i64,
    ) -> Result<u64, anyhow::Error>;

    async fn get_session_details(
        &self,
        appointment_id: &str,
    ) -> Result<Option<SessionDetails>, anyhow::Error>;
}

pub struct EndSessionRepoPsql {
    pool: PgPool,
}

impl EndSessionRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl EndSessionRepo for EndSessionRepoPsql {
    async fn complete_consultation_status_to_done(
        &self,
        appointment_id: &str,
        doctor_profile_id: i64,
    ) -> Result<u64, anyhow::Error> {
        let appointment_id: String =
            sqlx::query_scalar("SELECT appointment_id FROM v2.appointment WHERE booking_id = $1")
                .bind(appointment_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to resolve booking ID: {e}"))?
                .ok_or_else(|| anyhow::anyhow!("Booking not found"))?;
        sqlx::query_scalar::<_, i64>(COMPLETE_CONSULTATION_STATUS_TO_DONE_SQL)
            .bind(appointment_id)
            .bind(doctor_profile_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update appointment status: {}", e))
            .and_then(|rows| {
                u64::try_from(rows).map_err(|e| {
                    anyhow::anyhow!("Invalid rows affected from end_active_session: {e}")
                })
            })
    }

    async fn get_session_details(
        &self,
        appointment_id: &str,
    ) -> Result<Option<SessionDetails>, anyhow::Error> {
        let appointment_id: String =
            sqlx::query_scalar("SELECT appointment_id FROM v2.appointment WHERE booking_id = $1")
                .bind(appointment_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to resolve booking ID: {e}"))?
                .ok_or_else(|| anyhow::anyhow!("Booking not found"))?;
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complete_consultation_status_query_uses_v2_function() {
        assert!(COMPLETE_CONSULTATION_STATUS_TO_DONE_SQL.contains("v2.end_active_session"));
    }

    #[test]
    fn end_active_session_migration_guards_terminal_statuses_for_idempotency() {
        let migration = include_str!(
            "../../../../db/biz_apm/migrations/20260709000001__session_idempotency_guards.sql"
        );

        let active_status_guard = migration
            .split("a.appointment_status IN (")
            .nth(1)
            .and_then(|tail| tail.split(')').next())
            .expect("migration should guard end-session updates by active appointment statuses");

        assert!(active_status_guard.contains("'BOOKED'::v2.fhir_appointment_status_enum,"));
        assert!(active_status_guard.contains("'ARRIVED'::v2.fhir_appointment_status_enum"));
        assert!(!active_status_guard.contains("'FULFILLED'::v2.fhir_appointment_status_enum"));
        assert!(
            !active_status_guard.contains("'CONSULTATION_DONE'::v2.fhir_appointment_status_enum")
        );
    }
}
