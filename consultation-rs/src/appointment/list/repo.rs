use sqlx::PgPool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AppointmentListRow {
    pub booking_id: String,
    pub appointment_start: jiff_sqlx::Timestamp,
    pub appointment_end: jiff_sqlx::Timestamp,
    pub doctor_account_id: i32,
    pub doctor_profile_id: i32,
}

#[async_trait::async_trait]
pub trait ListAppointmentsRepo: Send + Sync {
    async fn list_fulfilled_appointments(
        &self,
        patient_account_id: i32,
        patient_profile_id: Option<i32>,
    ) -> Result<Vec<AppointmentListRow>, anyhow::Error>;
}

pub struct ListAppointmentsRepoPsql {
    pool: PgPool,
}

impl ListAppointmentsRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ListAppointmentsRepo for ListAppointmentsRepoPsql {
    async fn list_fulfilled_appointments(
        &self,
        patient_account_id: i32,
        patient_profile_id: Option<i32>,
    ) -> Result<Vec<AppointmentListRow>, anyhow::Error> {
        let rows = sqlx::query_as::<_, AppointmentListRow>(
            r#"SELECT * FROM v2.list_fulfilled_appointments_by_patient($1, $2)"#,
        )
        .bind(patient_account_id)
        .bind(patient_profile_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to list fulfilled appointments for patient_account_id={patient_account_id}: {e}"
            )
        })?;

        Ok(rows)
    }
}
