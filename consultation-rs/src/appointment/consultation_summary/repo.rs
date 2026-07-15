use sqlx::PgPool;

use crate::repo::enums::{AppointmentStatusEnum, ConsultationChannelEnum};

/// One joined row for a consultation-summary read. The `doctor_summary_note` columns are nullable
/// because the join is LEFT — a booking can exist (and even be FULFILLED) without a note,
/// which the service treats distinctly.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConsultationSummaryRow {
    pub booking_id: String,
    pub appointment_start: jiff_sqlx::Timestamp,
    pub appointment_end: jiff_sqlx::Timestamp,
    pub appointment_status: AppointmentStatusEnum,
    pub consultation_channel: ConsultationChannelEnum,
    pub doctor_id: i32,
    pub doctor_account_id: i32,
    pub doctor_profile_id: i32,
    pub encrypted_data: Option<String>,
    pub encrypted_data_type: Option<String>,
    pub prescription_id: Option<i64>,
}

#[async_trait::async_trait]
pub trait ConsultationSummaryRepo: Send + Sync {
    async fn get_consultation_summary(
        &self,
        booking_id: &str,
    ) -> Result<Option<ConsultationSummaryRow>, anyhow::Error>;
}

pub struct ConsultationSummaryRepoPsql {
    pool: PgPool,
}

impl ConsultationSummaryRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ConsultationSummaryRepo for ConsultationSummaryRepoPsql {
    async fn get_consultation_summary(
        &self,
        booking_id: &str,
    ) -> Result<Option<ConsultationSummaryRow>, anyhow::Error> {
        let row = sqlx::query_as::<_, ConsultationSummaryRow>(
            r#"
            SELECT
                h.booking_id,
                h.starts_at AS appointment_start,
                h.ends_at AS appointment_end,
                a.appointment_status,
                h.consultation_channel,
                h.doctor_id,
                h.doctor_account_id,
                h.doctor_profile_id,
                dsn.encrypted_data,
                dsn.encrypted_data_type,
                dsn.prescription_id
            FROM v2.appointment_hold h
            INNER JOIN v2.appointment a ON a.source_hold_id = h.hold_id
            LEFT JOIN v2.doctor_summary_note dsn ON dsn.appointment_id = a.appointment_id
            WHERE h.booking_id = $1
            "#,
        )
        .bind(booking_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            anyhow::anyhow!("Failed to get consultation summary for booking_id={booking_id}: {e}")
        })?;

        Ok(row)
    }
}
