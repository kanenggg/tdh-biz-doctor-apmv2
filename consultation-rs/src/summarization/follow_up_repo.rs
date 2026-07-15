use anyhow::Result;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::error;

use crate::repo::enums::AppointmentTypeEnum;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpCreationResult {
    pub booking_id: String,
    pub appointment_id: String,
    pub appointment_start: i64,
    pub appointment_end: i64,
    pub doctor_id: i32,
    pub doctor_profile_id: i32,
    pub consultation_channel: String,
    pub biz_unit_id: i32,
    pub biz_center_id: i32,
    pub tenant_id: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppointmentChainItem {
    pub appointment_id: String,
    pub parent_appointment_id: Option<String>,
    pub appointment_status: String,
    pub appointment_start: i64,
    pub appointment_end: i64,
    pub has_follow_up: bool,
    pub patient_account_id: i32,
    pub doctor_id: i32,
    pub biz_unit_id: i32,
    pub biz_center_id: i32,
}

#[derive(Clone)]
pub struct FollowUpRepoPsql {
    pool: PgPool,
}

impl FollowUpRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_follow_up(
        &self,
        parent_booking_id: &str,
        appointment_start: jiff_sqlx::Timestamp,
        consult_duration_seconds: i32,
        appointment_type: AppointmentTypeEnum,
    ) -> Result<FollowUpCreationResult> {
        let result: sqlx::types::Json<FollowUpCreationResult> = sqlx::query_scalar(
            r#"
            SELECT v2.create_follow_up_appointment(
                (SELECT appointment_id FROM v2.appointment WHERE booking_id = $1),
                $2::timestamptz,
                ($3 || ' seconds')::interval,
                $4::v2.appointment_type_enum
            )
            "#,
        )
        .bind(parent_booking_id)
        .bind(appointment_start)
        .bind(consult_duration_seconds)
        .bind(appointment_type)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!(
                error = %e,
                parent_booking_id = %parent_booking_id,
                "Failed to create follow-up appointment in database"
            );
            anyhow::anyhow!("Failed to create follow-up appointment: {}", e)
        })?;

        Ok(result.0)
    }

    pub async fn mark_has_follow_up(&self, appointment_id: &str) -> Result<bool> {
        let success: bool = sqlx::query_scalar(
            r#"
            SELECT v2.mark_appointment_has_follow_up($1::varchar)
            "#,
        )
        .bind(appointment_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!(
                error = %e,
                appointment_id = %appointment_id,
                "Failed to mark appointment as having follow-up"
            );
            anyhow::anyhow!("Failed to mark appointment has follow-up: {}", e)
        })?;

        Ok(success)
    }

    pub async fn get_appointment_chain(
        &self,
        appointment_id: &str,
    ) -> Result<Vec<AppointmentChainItem>> {
        let result: sqlx::types::Json<Vec<AppointmentChainItem>> = sqlx::query_scalar(
            r#"
            SELECT v2.get_appointment_chain($1::varchar)
            "#,
        )
        .bind(appointment_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!(
                error = %e,
                appointment_id = %appointment_id,
                "Failed to get appointment chain from database"
            );
            anyhow::anyhow!("Failed to get appointment chain: {}", e)
        })?;

        Ok(result.0)
    }
}
