use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentMessage {
    pub payment_status: String,
    pub payment_data: String,
    pub payment_ref_code: String,
    pub payment_transaction_ref_id: String,
}
