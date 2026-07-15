use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::coupon::CouponProtocol;
use super::delivery::DeliveryOptionInfoProtocol;
use super::payment_summary::PaymentSummary;
use super::selected_channel_result::SelectedChannelResult;

// Note: UserIdentity is imported from the protocol module
// You may need to adjust this import based on your actual project structure
// use crate::tdh_protocol::user_identity::UserIdentity;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentTransaction {
    #[serde(rename = "paymentTransactionId")]
    pub payment_transaction_id: i64,
    #[serde(rename = "paymentTransactionRefId")]
    pub payment_transaction_ref_id: String,
    // Uncomment and adjust when UserIdentity is available
    // #[serde(rename = "userIdentity")]
    // pub user_identity: UserIdentity,
    #[serde(rename = "ackRefKey")]
    pub ack_ref_key: String,
    #[serde(rename = "paymentSummary")]
    pub payment_summary: PaymentSummary,
    pub amount: Decimal,
    #[serde(rename = "orderTotal")]
    pub order_total: Decimal,
    #[serde(rename = "orderGrandTotal")]
    pub order_grand_total: Decimal,
    #[serde(rename = "platformFee")]
    pub platform_fee: Decimal,
    #[serde(rename = "selectedChannelResult")]
    pub selected_channel_result: Option<SelectedChannelResult>,
    #[serde(rename = "deliveryInfoV2")]
    pub delivery_info_v2: Option<DeliveryOptionInfoProtocol>,
    #[serde(rename = "couponProtocol")]
    pub coupon_protocol: Option<CouponProtocol>,
    #[serde(rename = "paymentStatus")]
    pub payment_status: Option<String>,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "modifiedAt")]
    pub modified_at: i64,
}
