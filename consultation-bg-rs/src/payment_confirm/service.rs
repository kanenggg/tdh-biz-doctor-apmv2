use std::sync::Arc;

use crate::common::tdh_protocol::{
    appointment::v2::payment_transaction::{PaymentChannel, PaymentChannels},
    payment::{
        payment_channel_result::PaymentChannelResult, payment_message::PaymentMessage,
        payment_transaction::PaymentTransaction, selected_channel_result::SelectedChannelResult,
    },
};
use crate::payment_confirm::{
    payement_token::PaymentTokenVerifier,
    repo::{
        PaymentConfirmDomainError, PaymentConfirmRepo, PaymentConfirmRepoError, PaymentConfirmation,
    },
};

#[derive(thiserror::Error, Debug)]
pub enum PaymentConfirmError {
    #[error("signed payment transaction is not successful")]
    SignedPaymentNotSuccessful,
    #[error("outer payment message does not match the signed payment transaction")]
    OuterMessageMismatch,
    #[error("unexpected extended data type")]
    UnexpectedExtendData,
    #[error("missing extended data")]
    MissingExtendData,
    #[error(
        "FollowUp payment is unsupported until the payment contract supplies its new bookingId"
    )]
    MissingFollowUpBookingId,
    #[error("invalid payment token: {0}")]
    InvalidToken(String),
    #[error("payment confirmation rejected: {0:?}")]
    Domain(PaymentConfirmDomainError),
    #[error("payment confirmation database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("payment confirmation serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl From<PaymentConfirmRepoError> for PaymentConfirmError {
    fn from(error: PaymentConfirmRepoError) -> Self {
        match error {
            PaymentConfirmRepoError::Domain(error) => Self::Domain(error),
            PaymentConfirmRepoError::Database(error) => Self::Database(error),
            PaymentConfirmRepoError::Serialization(error) => Self::Serialization(error),
        }
    }
}

pub struct PaymentConfirmService {
    payment_verifier: Arc<dyn PaymentTokenVerifier>,
    repo: Arc<dyn PaymentConfirmRepo>,
    consultation_event_topic: Option<String>,
}

impl PaymentConfirmService {
    pub fn new(
        payment_verifier: Arc<dyn PaymentTokenVerifier>,
        repo: Arc<dyn PaymentConfirmRepo>,
        consultation_event_topic: Option<String>,
    ) -> Self {
        Self {
            payment_verifier,
            repo,
            consultation_event_topic,
        }
    }

    /// Confirms successful payment through exactly one atomic repository call.
    pub async fn handle_payment_tx_v1(
        &self,
        msg: PaymentMessage,
    ) -> Result<(), PaymentConfirmError> {
        let consultation_event_topic = self.consultation_event_topic.clone().ok_or_else(|| {
            PaymentConfirmError::InvalidToken(
                "Consultation V1 event publishing is disabled by rollout mode".to_string(),
            )
        })?;
        let transaction = self
            .payment_verifier
            .verify(&msg.payment_data)
            .map_err(|error| PaymentConfirmError::InvalidToken(error.to_string()))?;
        self.verify_outer_message(&msg, &transaction)?;
        let confirmation = self.payment_confirmation(transaction, consultation_event_topic)?;

        tracing::info!(booking_id = %confirmation.booking_id, payment_tx_id = confirmation.payment_tx_id, "Confirming payment atomically");
        self.repo.confirm_payment(confirmation).await?;
        Ok(())
    }

    /// Pub/Sub envelope fields are untrusted routing metadata.  The signed
    /// transaction is authoritative, and every duplicated fact must agree
    /// before the database booking transaction can run.
    fn verify_outer_message(
        &self,
        message: &PaymentMessage,
        transaction: &PaymentTransaction,
    ) -> Result<(), PaymentConfirmError> {
        if !matches!(transaction.payment_status.as_deref(), Some(status) if status.eq_ignore_ascii_case("success"))
        {
            return Err(PaymentConfirmError::SignedPaymentNotSuccessful);
        }
        if !message.payment_status.eq_ignore_ascii_case("success")
            || message.payment_ref_code != transaction.payment_summary.request.ref_code
            || message.payment_transaction_ref_id != transaction.payment_transaction_ref_id
            || transaction.ack_ref_key != transaction.payment_summary.request.ref_code
        {
            return Err(PaymentConfirmError::OuterMessageMismatch);
        }
        Ok(())
    }

    fn payment_confirmation(
        &self,
        transaction: PaymentTransaction,
        consultation_event_topic: String,
    ) -> Result<PaymentConfirmation, PaymentConfirmError> {
        let booking_id = self.extract_booking_id(&transaction)?;

        let payment_channels = self.extract_payment_channels(&transaction);
        Ok(PaymentConfirmation {
            booking_id,
            payment_tx_id: transaction.payment_transaction_id,
            payment_tx_ref_id: transaction.payment_transaction_ref_id,
            payment_channels,
            amount: transaction
                .payment_summary
                .request
                .amount
                .normalize()
                .to_string(),
            currency: transaction.payment_summary.request.currency,
            payment_module_id: transaction
                .payment_summary
                .request
                .module_id
                .unwrap_or_default(),
            booked_at: if transaction.modified_at > 0 {
                transaction.modified_at
            } else {
                transaction.created_at
            },
            consultation_event_topic,
        })
    }

    fn extract_booking_id(&self, tx: &PaymentTransaction) -> Result<String, PaymentConfirmError> {
        let extend_data = tx
            .payment_summary
            .request
            .extended_data
            .as_ref()
            .ok_or(PaymentConfirmError::MissingExtendData)?;
        match extend_data {
            crate::common::tdh_protocol::payment::extend_data::ExtendData::ConsultInfo {
                booking_id,
                ..
            }
            | crate::common::tdh_protocol::payment::extend_data::ExtendData::ConsultInfoV2 {
                booking_id,
            } => Ok(booking_id.clone()),
            crate::common::tdh_protocol::payment::extend_data::ExtendData::FollowUp {
                ..
            // `previousBookingId` identifies the completed consultation, not
            // the new payment/hold.  The committed payment contract has not
            // supplied the required `bookingId` yet, so booking must fail
            // closed rather than silently charge/book the old appointment.
            } => Err(PaymentConfirmError::MissingFollowUpBookingId),
            _ => Err(PaymentConfirmError::UnexpectedExtendData),
        }
    }

    fn extract_payment_channels(&self, tx: &PaymentTransaction) -> PaymentChannels {
        let Some(channel_result) = tx.selected_channel_result.as_ref() else {
            return vec![];
        };
        match channel_result {
            SelectedChannelResult::CoverageChannel { channel } => {
                Self::channel_result_to_channel(channel)
                    .into_iter()
                    .collect()
            }
            SelectedChannelResult::SelfPayChannel { channel } => {
                Self::channel_result_to_channel(channel)
                    .into_iter()
                    .collect()
            }
            SelectedChannelResult::CoverageAndSelfPayChannel {
                coverage_channel,
                self_pay_channel,
            } => [
                Self::channel_result_to_channel(coverage_channel),
                Self::channel_result_to_channel(self_pay_channel),
            ]
            .into_iter()
            .flatten()
            .collect(),
        }
    }

    fn channel_result_to_channel(result: &PaymentChannelResult) -> Option<PaymentChannel> {
        match result {
            PaymentChannelResult::InsuranceV3 {
                binding_id,
                privilege_id,
                ..
            } => Some(PaymentChannel::Insurance {
                binding_id: *binding_id,
                privilege_id: *privilege_id,
            }),
            PaymentChannelResult::Insurance { policy_id, .. } => Some(PaymentChannel::Insurance {
                binding_id: i64::from(*policy_id),
                privilege_id: 0,
            }),
            PaymentChannelResult::InsuranceV2 { binding_id, .. } => {
                Some(PaymentChannel::Insurance {
                    binding_id: i64::from(*binding_id),
                    privilege_id: 0,
                })
            }
            PaymentChannelResult::EmployeeBenefit { .. }
            | PaymentChannelResult::EmployeeBenefitV2 { .. } => {
                Some(PaymentChannel::EmployeeBenefit {})
            }
            PaymentChannelResult::CampaignLocation { .. } => {
                Some(PaymentChannel::CampaignLocation {})
            }
            PaymentChannelResult::Campaign { .. } => Some(PaymentChannel::Campaign),
            PaymentChannelResult::Card { id, .. } => Some(PaymentChannel::Card { id: id.clone() }),
            PaymentChannelResult::PromptPay { id, .. } => {
                Some(PaymentChannel::PromptPay { id: id.clone() })
            }
            PaymentChannelResult::TrueMoney { id, .. } => {
                Some(PaymentChannel::TrueMoney { id: id.clone() })
            }
            PaymentChannelResult::Wallet { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct FixedVerifier(PaymentTransaction);
    impl PaymentTokenVerifier for FixedVerifier {
        fn verify(&self, _: &str) -> Result<PaymentTransaction, anyhow::Error> {
            Ok(self.0.clone())
        }
    }

    #[derive(Default)]
    struct RecordingRepo(Mutex<Vec<PaymentConfirmation>>);
    #[async_trait::async_trait]
    impl PaymentConfirmRepo for RecordingRepo {
        async fn confirm_payment(
            &self,
            input: PaymentConfirmation,
        ) -> Result<(), PaymentConfirmRepoError> {
            self.0.lock().expect("recording mutex poisoned").push(input);
            Ok(())
        }
    }

    fn payment_transaction() -> PaymentTransaction {
        serde_json::from_value(serde_json::json!({
            "paymentTransactionId": 1042, "paymentTransactionRefId": "PT-2026-001", "ackRefKey": "booking-1",
            "paymentSummary": {"header": {"bizUnitId": 7, "bizCenterId": 1, "flowId": 1}, "request": {
                "moduleId": 12, "refCode": "booking-1", "amount": "1500.00", "currency": "THB",
                "pricePlanId": 1, "isRequireDelivery": false,
                "extendedData": {"__type": "ExtendData.ConsultInfoV2", "bookingId": "booking-1", "doctorId": "doctor-300",
                    "departmentId": 1, "clinicIds": [1], "medicalSpecialtyId": 1, "channel": "video", "scheduleTime": 1772179200,
                    "doctorOriginalFee": "1500.00", "platformFee": {"amount": "0", "currency": "THB"},
                    "onlySelfPayChannelShown": false}
            }}, "amount": "1500.00", "orderTotal": "1500.00", "orderGrandTotal": "1500.00", "platformFee": "0",
            "selectedChannelResult": null, "deliveryInfoV2": null, "couponProtocol": null,
            "paymentStatus": "success", "createdAt": 1772179100, "modifiedAt": 1772179150
        })).expect("valid payment transaction fixture")
    }

    fn success_message() -> PaymentMessage {
        PaymentMessage {
            payment_status: "success".into(),
            payment_data: "signed".into(),
            payment_ref_code: "booking-1".into(),
            payment_transaction_ref_id: "PT-2026-001".into(),
        }
    }

    #[tokio::test]
    async fn payment_confirmation_uses_one_atomic_repository_call_with_exact_quote_facts() {
        let repo = Arc::new(RecordingRepo::default());
        let service = PaymentConfirmService::new(
            Arc::new(FixedVerifier(payment_transaction())),
            repo.clone(),
            Some("consultation-events".into()),
        );
        service
            .handle_payment_tx_v1(success_message())
            .await
            .expect("payment confirmation succeeds");
        let calls = repo.0.lock().expect("recording mutex poisoned");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].amount, "1500");
        assert_eq!(calls[0].currency, "THB");
    }

    #[tokio::test]
    async fn signed_failed_transaction_cannot_be_booked_when_outer_envelope_claims_success() {
        let repo = Arc::new(RecordingRepo::default());
        let mut transaction = payment_transaction();
        transaction.payment_status = Some("failed".to_string());
        let service = PaymentConfirmService::new(
            Arc::new(FixedVerifier(transaction)),
            repo.clone(),
            Some("consultation-events".into()),
        );

        let error = service
            .handle_payment_tx_v1(success_message())
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            PaymentConfirmError::SignedPaymentNotSuccessful
        ));
        assert!(repo.0.lock().expect("recording mutex poisoned").is_empty());
    }

    #[tokio::test]
    async fn mismatched_outer_reference_cannot_be_booked() {
        let repo = Arc::new(RecordingRepo::default());
        let service = PaymentConfirmService::new(
            Arc::new(FixedVerifier(payment_transaction())),
            repo.clone(),
            Some("consultation-events".into()),
        );
        let mut message = success_message();
        message.payment_transaction_ref_id = "substituted-reference".to_string();

        let error = service.handle_payment_tx_v1(message).await.unwrap_err();
        assert!(matches!(error, PaymentConfirmError::OuterMessageMismatch));
        assert!(repo.0.lock().expect("recording mutex poisoned").is_empty());
    }

    #[tokio::test]
    async fn follow_up_previous_booking_id_is_never_used_as_the_new_booking_id() {
        let repo = Arc::new(RecordingRepo::default());
        let mut transaction = payment_transaction();
        transaction.payment_summary.request.extended_data = Some(
            serde_json::from_value(serde_json::json!({
                "__type": "ExtendData.FollowUp", "previousBookingId": "old-booking",
                "onlySelfPayChannelShown": true, "doctorId": 300, "doctorUsId": "doctor-us-1",
                "channel": "video", "scheduleTime": 1772179200
            }))
            .unwrap(),
        );
        let service = PaymentConfirmService::new(
            Arc::new(FixedVerifier(transaction)),
            repo.clone(),
            Some("consultation-events".into()),
        );
        let error = service
            .handle_payment_tx_v1(success_message())
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            PaymentConfirmError::MissingFollowUpBookingId
        ));
        assert!(repo.0.lock().expect("recording mutex poisoned").is_empty());
    }
}
