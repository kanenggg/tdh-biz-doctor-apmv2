use sqlx::{Pool, Postgres};

pub struct FacialUploadRepoPsql {
    pub pg_pool: Pool<Postgres>,
}

impl FacialUploadRepoPsql {
    pub fn new(pg_pool: Pool<Postgres>) -> Self {
        Self { pg_pool }
    }

    pub async fn get_appointment_detail(
        &self,
        booking_id: &str,
        role: &str,
        profile_id: i64,
    ) -> Result<Option<String>, anyhow::Error> {
        let result = if role == "patient" {
            sqlx::query_scalar::<_, String>(
                r#"
                SELECT appointment_id FROM v2.appointment
                WHERE booking_id = $1
                  AND patient_profile_id = $2
                  AND appointment_status IN ('BOOKED'::v2.fhir_appointment_status_enum,
                    'ARRIVED'::v2.fhir_appointment_status_enum)
                "#,
            )
            .bind(booking_id)
            .bind(profile_id)
            .fetch_optional(&self.pg_pool)
            .await
        } else {
            sqlx::query_scalar::<_, String>(
                r#"
                SELECT appointment_id FROM v2.appointment
                WHERE booking_id = $1
                  AND doctor_profile_id = $2
                  AND appointment_status IN ('BOOKED'::v2.fhir_appointment_status_enum,
                    'ARRIVED'::v2.fhir_appointment_status_enum)
                "#,
            )
            .bind(booking_id)
            .bind(profile_id)
            .fetch_optional(&self.pg_pool)
            .await
        };

        result.map_err(|e| anyhow::anyhow!("Failed to get appointment detail: {}", e))
    }

    pub async fn insert_facial_upload(
        &self,
        appointment_id: &str,
        user_profile_id: i32,
        user_account_id: i32,
        object_url: &str,
    ) -> Result<(), anyhow::Error> {
        sqlx::query(
            r#"
            INSERT INTO v2.appointment_facial_upload (
                appointment_id, user_profile_id, user_account_id, object_url
            ) VALUES ($1, $2, $3, $4)
            ON CONFLICT (appointment_id) DO UPDATE SET
                object_url = EXCLUDED.object_url,
                created_at = NOW()
            "#,
        )
        .bind(appointment_id)
        .bind(user_profile_id)
        .bind(user_account_id)
        .bind(object_url)
        .execute(&self.pg_pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to insert facial upload: {}", e))?;

        Ok(())
    }
}
