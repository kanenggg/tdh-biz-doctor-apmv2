use crate::common::tdh_protocol::appointment::v2::payment_transaction::PaymentChannels;
use sqlx::{Pool, Postgres};
use tracing::instrument;

/// Facts authenticated by the payment token.  The database compares these to
/// the Hold's immutable quote before it changes any booking state.
#[derive(Debug, Clone, PartialEq)]
pub struct PaymentConfirmation {
    pub booking_id: String,
    pub payment_tx_id: i64,
    pub payment_tx_ref_id: String,
    pub payment_channels: PaymentChannels,
    pub amount: String,
    pub currency: String,
    pub payment_module_id: i32,
    pub booked_at: i64,
    pub consultation_event_topic: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentConfirmDomainError {
    HoldNotFound,
    HoldNotBookable,
    ReplayConflict,
    QuoteMismatch,
    MissingOccupancy,
    InvalidRequest,
}

#[derive(Debug, thiserror::Error)]
pub enum PaymentConfirmRepoError {
    #[error("payment confirmation domain error: {0:?}")]
    Domain(PaymentConfirmDomainError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error("payment confirmation serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl PaymentConfirmRepoError {
    fn from_sqlx(error: sqlx::Error) -> Self {
        let domain_error =
            error.as_database_error().and_then(|database_error| {
                match database_error.code().as_deref() {
                    Some("P1001") | Some("P1004") => {
                        Some(PaymentConfirmDomainError::HoldNotBookable)
                    }
                    Some("P1002") => Some(PaymentConfirmDomainError::ReplayConflict),
                    Some("P1003") => Some(PaymentConfirmDomainError::MissingOccupancy),
                    Some("P1005") => Some(PaymentConfirmDomainError::QuoteMismatch),
                    Some("P0002") => Some(PaymentConfirmDomainError::HoldNotFound),
                    Some("22023") => Some(PaymentConfirmDomainError::InvalidRequest),
                    _ => None,
                }
            });
        domain_error.map_or(Self::Database(error), Self::Domain)
    }
}

#[async_trait::async_trait]
pub trait PaymentConfirmRepo: Send + Sync {
    /// Runs the complete confirmation and outbox insert in one database call.
    async fn confirm_payment(
        &self,
        confirmation: PaymentConfirmation,
    ) -> Result<(), PaymentConfirmRepoError>;
}

pub struct PaymentConfirmPsql {
    pool: Pool<Postgres>,
}

impl PaymentConfirmPsql {
    pub fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl PaymentConfirmRepo for PaymentConfirmPsql {
    #[instrument(skip(self, confirmation), fields(booking_id = %confirmation.booking_id))]
    async fn confirm_payment(
        &self,
        confirmation: PaymentConfirmation,
    ) -> Result<(), PaymentConfirmRepoError> {
        let payment_channels = serde_json::to_value(&confirmation.payment_channels)?;

        sqlx::query(
            "SELECT v2.confirm_payment_and_enqueue_consultation_booked($1, $2, $3, $4, $5::numeric, $6, $7, $8, $9)",
        )
        .bind(&confirmation.booking_id)
        .bind(confirmation.payment_tx_id)
        .bind(&confirmation.payment_tx_ref_id)
        .bind(payment_channels)
        .bind(&confirmation.amount)
        .bind(&confirmation.currency)
        .bind(confirmation.payment_module_id)
        .bind(confirmation.booked_at)
        .bind(&confirmation.consultation_event_topic)
        .execute(&self.pool)
        .await
        .map_err(PaymentConfirmRepoError::from_sqlx)?;

        Ok(())
    }
}
