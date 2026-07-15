use sqlx::{Pool, Postgres};

const ADD_PATIENT_ID_VERIFICATION_SQL: &str = r#"
SELECT v2.add_patient_id_verification($1, $2)
"#;

const CANCEL_APPOINTMENT_SQL: &str = r#"
SELECT v2.cancel_appointment($1, $2)
"#;

pub struct PatientVerificationRepoPsql {
    pub pg_pool: Pool<Postgres>,
}

impl PatientVerificationRepoPsql {
    pub fn new(pg_pool: Pool<Postgres>) -> Self {
        Self { pg_pool }
    }

    pub async fn add_patient_verification(
        &self,
        booking_id: &str,
        doctor_id: i64,
    ) -> Result<u64, anyhow::Error> {
        sqlx::query_scalar::<_, i64>(ADD_PATIENT_ID_VERIFICATION_SQL)
            .bind(booking_id)
            .bind(doctor_id)
            .fetch_one(&self.pg_pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update appointment status: {}", e))
            .and_then(rows_returned_by_patient_verification_function)
    }

    pub async fn canncel_appointment(
        &self,
        booking_id: &str,
        doctor_id: i64,
    ) -> Result<u64, anyhow::Error> {
        sqlx::query_scalar::<_, i64>(CANCEL_APPOINTMENT_SQL)
            .bind(booking_id)
            .bind(doctor_id)
            .fetch_one(&self.pg_pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to update appointment status: {}", e))
            .and_then(rows_returned_by_patient_verification_function)
    }
}

fn rows_returned_by_patient_verification_function(rows: i64) -> Result<u64, anyhow::Error> {
    u64::try_from(rows).map_err(|e| {
        anyhow::anyhow!("Invalid rows affected from patient verification function: {e}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_patient_verification_query_reads_function_return_value() {
        assert!(ADD_PATIENT_ID_VERIFICATION_SQL.contains("v2.add_patient_id_verification"));
    }

    #[test]
    fn cancel_appointment_query_reads_v2_function_return_value() {
        assert!(CANCEL_APPOINTMENT_SQL.contains("v2.cancel_appointment"));
    }

    #[test]
    fn converts_database_function_rows_to_unsigned_count() {
        assert_eq!(
            rows_returned_by_patient_verification_function(0).unwrap(),
            0
        );
        assert_eq!(
            rows_returned_by_patient_verification_function(1).unwrap(),
            1
        );
        assert!(rows_returned_by_patient_verification_function(-1).is_err());
    }
}
