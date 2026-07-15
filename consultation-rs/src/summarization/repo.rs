use crate::protocol::PatientIdentity;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::error;

#[derive(Clone)]
pub struct CreateSummaryNoteParams {
    /// Public booking correlation; the repository resolves it to Appointment ID.
    pub booking_id: String,
    pub encrypted_data: String,
    pub encrypted_data_type: String,
    pub note_to_staff: Option<String>,
    pub icd10_codes: Vec<String>,
    pub prescription_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSummaryNoteResult {
    pub created: bool,
    pub summary_note_id: i64,
    pub patient_account_id: i32,
    pub user_profile_id: i32,
    pub tenant_id: i32,
    pub biz_unit_id: i64,
    pub biz_center_id: i64,
}

impl CreateSummaryNoteResult {
    pub fn patient_identity(&self) -> PatientIdentity {
        PatientIdentity {
            account_id: self.patient_account_id,
            user_profile_id: self.user_profile_id,
            tenant_id: self.tenant_id,
            oidc_user_id: None,
        }
    }
}

pub struct SummaryNoteRepoPsql {
    pool: PgPool,
}

impl SummaryNoteRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert(
        &self,
        params: CreateSummaryNoteParams,
    ) -> Result<CreateSummaryNoteResult, anyhow::Error> {
        let booking_id = params.booking_id.clone();
        let result: sqlx::types::Json<CreateSummaryNoteResult> = sqlx::query_scalar(
            r#"
            SELECT v2.create_if_not_existing_summary_note(
                (SELECT appointment_id FROM v2.appointment WHERE booking_id = $1),
                $2, $3, $4, $5, $6
            )
            "#,
        )
        .bind(params.booking_id)
        .bind(params.encrypted_data)
        .bind(params.encrypted_data_type)
        .bind(params.note_to_staff)
        .bind(sqlx::types::Json(&params.icd10_codes))
        .bind(params.prescription_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            error!(
                error = %e,
                booking_id = %booking_id,
                "Failed to insert summary note into database"
            );
            anyhow::anyhow!("Failed to insert summary note: {}", e)
        })?;

        Ok(result.0)
    }
}
