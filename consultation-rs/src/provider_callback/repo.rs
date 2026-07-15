use sqlx::{FromRow, PgPool};

use crate::consultation::common::{DbSessionDetails, SessionDetails};
use crate::repo::provider_session_info::{SessionData, TwilioSessionInfo};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackParticipantRole {
    Patient,
    Doctor,
}

pub struct TwilioCallbackSessionContext {
    pub details: SessionDetails,
    pub room_sid: Option<String>,
}

#[derive(FromRow)]
struct DbTwilioCallbackSessionContext {
    appointment_id: String,
    booking_id: String,
    patient_account_id: i32,
    patient_profile_id: i32,
    tenant_id: i32,
    doctor_id: i32,
    doctor_profile_id: i32,
    session_provider: String,
    session_chat_id: Option<String>,
    session_data: Option<serde_json::Value>,
}

#[async_trait::async_trait]
pub trait ProviderCallbackRepo: Send + Sync {
    async fn insert_callback_event(
        &self,
        provider_event_id: &str,
        appointment_id: Option<&str>,
        event_type: &str,
        participant_identity: Option<&str>,
        payload: serde_json::Value,
    ) -> Result<bool, anyhow::Error>;

    async fn mark_participant_disconnected(
        &self,
        appointment_id: &str,
        role: CallbackParticipantRole,
        disconnected_at: i64,
    ) -> Result<bool, anyhow::Error>;

    async fn get_session_details(
        &self,
        appointment_id: &str,
    ) -> Result<Option<SessionDetails>, anyhow::Error>;

    async fn get_twilio_callback_context(
        &self,
        appointment_id: &str,
    ) -> Result<Option<TwilioCallbackSessionContext>, anyhow::Error>;
}

pub struct ProviderCallbackRepoPsql {
    pool: PgPool,
}

impl ProviderCallbackRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ProviderCallbackRepo for ProviderCallbackRepoPsql {
    async fn insert_callback_event(
        &self,
        provider_event_id: &str,
        appointment_id: Option<&str>,
        event_type: &str,
        participant_identity: Option<&str>,
        payload: serde_json::Value,
    ) -> Result<bool, anyhow::Error> {
        let inserted = sqlx::query_scalar::<_, bool>(
            r#"
            INSERT INTO v2.provider_callback_event (
                provider,
                provider_event_id,
                appointment_id,
                event_type,
                participant_identity,
                payload
            ) VALUES ('TWILIO', $1, $2, $3, $4, $5)
            ON CONFLICT (provider, provider_event_id) DO NOTHING
            RETURNING true
            "#,
        )
        .bind(provider_event_id)
        .bind(appointment_id)
        .bind(event_type)
        .bind(participant_identity)
        .bind(payload)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to insert provider callback event: {e}"))?;

        Ok(inserted.unwrap_or(false))
    }

    async fn mark_participant_disconnected(
        &self,
        appointment_id: &str,
        role: CallbackParticipantRole,
        disconnected_at: i64,
    ) -> Result<bool, anyhow::Error> {
        let sql = match role {
            CallbackParticipantRole::Patient => {
                r#"
                UPDATE v2.session_info
                SET patient_disconnected_at = to_timestamp($2),
                    modified_at = NOW()
                WHERE appointment_id = $1
                  AND patient_disconnected_at IS NULL
                RETURNING true
                "#
            }
            CallbackParticipantRole::Doctor => {
                r#"
                UPDATE v2.session_info
                SET doctor_disconnected_at = to_timestamp($2),
                    modified_at = NOW()
                WHERE appointment_id = $1
                  AND doctor_disconnected_at IS NULL
                RETURNING true
                "#
            }
        };

        let marked = sqlx::query_scalar::<_, bool>(sql)
            .bind(appointment_id)
            .bind(disconnected_at as f64)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to mark participant disconnected: {e}"))?;

        Ok(marked.unwrap_or(false))
    }

    async fn get_session_details(
        &self,
        appointment_id: &str,
    ) -> Result<Option<SessionDetails>, anyhow::Error> {
        sqlx::query_as::<_, DbSessionDetails>(
            r#"
            SELECT * FROM v2.get_session_details($1)
            "#,
        )
        .bind(appointment_id)
        .fetch_optional(&self.pool)
        .await
        .map(|opt| opt.map(|db| db.into()))
        .map_err(|e| anyhow::anyhow!("Failed to get session details: {e}"))
    }

    async fn get_twilio_callback_context(
        &self,
        appointment_id: &str,
    ) -> Result<Option<TwilioCallbackSessionContext>, anyhow::Error> {
        let row = sqlx::query_as::<_, DbTwilioCallbackSessionContext>(
            r#"
            SELECT details.appointment_id,
                   details.booking_id,
                   details.patient_account_id,
                   details.patient_profile_id,
                   details.tenant_id,
                   details.doctor_id,
                   details.doctor_profile_id,
                   details.session_provider,
                   details.session_chat_id,
                   si.session_data
            FROM v2.get_session_details($1) AS details
            LEFT JOIN v2.session_info si ON si.appointment_id = details.appointment_id
            "#,
        )
        .bind(appointment_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to get Twilio callback context: {e}"))?;

        let Some(row) = row else {
            return Ok(None);
        };
        let details = DbSessionDetails {
            appointment_id: row.appointment_id,
            booking_id: row.booking_id,
            patient_account_id: row.patient_account_id,
            patient_profile_id: row.patient_profile_id,
            tenant_id: row.tenant_id,
            doctor_id: row.doctor_id,
            doctor_profile_id: row.doctor_profile_id,
            session_provider: row.session_provider,
            session_chat_id: row.session_chat_id,
        };
        let twilio_info = row
            .session_data
            .and_then(|value| serde_json::from_value::<SessionData>(value).ok())
            .and_then(|session_data| match session_data {
                SessionData::Twilio(info) => Some(info),
                SessionData::TokBox(_) => None,
            });

        Ok(Some(TwilioCallbackSessionContext {
            details: details.into(),
            room_sid: twilio_info.and_then(|info: TwilioSessionInfo| info.session_room_id),
        }))
    }
}
