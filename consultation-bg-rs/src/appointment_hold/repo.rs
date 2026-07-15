use sqlx::PgPool;

#[async_trait::async_trait]
pub(crate) trait AppointmentHoldExpiryRepo: Send + Sync {
    async fn expire_due_holds(&self, batch_size: i32, topic: &str) -> Result<usize, sqlx::Error>;
}

pub(crate) struct AppointmentHoldExpiryPsql {
    pool: PgPool,
}
impl AppointmentHoldExpiryPsql {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl AppointmentHoldExpiryRepo for AppointmentHoldExpiryPsql {
    async fn expire_due_holds(&self, batch_size: i32, topic: &str) -> Result<usize, sqlx::Error> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT booking_id FROM v2.expire_appointment_holds($1, $2)")
                .bind(batch_size.max(1))
                .bind(topic)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.len())
    }
}
