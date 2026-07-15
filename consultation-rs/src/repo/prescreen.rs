use sqlx::PgPool;

#[async_trait::async_trait]
pub trait PrescreenRepo: Send + Sync {
    /// Insert an encrypted patient intake record.
    ///
    /// `appointment_id` is set to `reservation_id` at booking time (the FK to `appointment` was
    /// dropped by migration `20240101000004_drop_prescreen_fk.sql` so this is safe before payment).
    async fn insert_prescreen(
        &self,
        appointment_id: i64,
        user_account_id: i32,
        user_profile_id: i32,
        encrypted_data: &str,
        encrypted_data_type: &str,
    ) -> Result<i64, anyhow::Error>;
}

pub struct PrescreenRepoPsql {
    pool: PgPool,
}

impl PrescreenRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl PrescreenRepo for PrescreenRepoPsql {
    async fn insert_prescreen(
        &self,
        appointment_id: i64,
        user_account_id: i32,
        user_profile_id: i32,
        encrypted_data: &str,
        encrypted_data_type: &str,
    ) -> Result<i64, anyhow::Error> {
        let prescreen_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO v2.patient_prescreen
                (booking_id, user_account_id, user_profile_id,
                 encrypted_data, encrypted_data_type)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING prescreen_id
            "#,
        )
        .bind(appointment_id)
        .bind(user_account_id)
        .bind(user_profile_id)
        .bind(encrypted_data)
        .bind(encrypted_data_type)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to insert patient prescreen: {}", e))?;

        Ok(prescreen_id)
    }
}
