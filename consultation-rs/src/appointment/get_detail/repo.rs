use sqlx::PgPool;

use crate::repo::enums::{AppointmentStatusEnum, BookingTypeEnum, ConsultationChannelEnum};

#[derive(Debug, sqlx::FromRow)]
pub struct AppointmentDetailRow {
    pub booking_id: String,
    pub appointment_start: jiff_sqlx::Timestamp,
    pub appointment_end: jiff_sqlx::Timestamp,
    pub appointment_status: AppointmentStatusEnum,
    pub booking_type: BookingTypeEnum,
    pub consultation_channel: ConsultationChannelEnum,
    pub patient_account_id: i32,
    pub patient_profile_id: i32,
    pub doctor_account_id: i32,
    pub doctor_profile_id: i32,
    pub prescreen_data: String,
    pub prescreen_data_type: String,
    pub payment_tx_id: i64,
    pub payment_tx_ref_id: String,
}

#[async_trait::async_trait]
pub trait GetAppointmentDetailRepo: Send + Sync {
    async fn get_appointment_detail(
        &self,
        booking_id: &str,
    ) -> Result<Option<AppointmentDetailRow>, anyhow::Error>;
}

pub struct GetAppointmentDetailRepoPsql {
    pool: PgPool,
}

impl GetAppointmentDetailRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl GetAppointmentDetailRepo for GetAppointmentDetailRepoPsql {
    async fn get_appointment_detail(
        &self,
        booking_id: &str,
    ) -> Result<Option<AppointmentDetailRow>, anyhow::Error> {
        let row = sqlx::query_as::<_, AppointmentDetailRow>(
            r#"SELECT * FROM v2.get_appointment_detail($1)"#,
        )
        .bind(booking_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            anyhow::anyhow!("Failed to get appointment detail for booking_id={booking_id}: {e}")
        })?;

        Ok(row)
    }
}
