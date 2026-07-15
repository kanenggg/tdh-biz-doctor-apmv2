use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::address_detail::AddressDetail;
use super::address_vendor_detail::AddressVendorDetail;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeliveryOptionInfoProtocol {
    #[serde(rename = "addressId")]
    pub address_id: i32,
    #[serde(rename = "ackRefKey")]
    pub ack_ref_key: String,
    #[serde(rename = "recipientAddress")]
    pub recipient_address: AddressDetail,
    #[serde(rename = "vendorAddress")]
    pub vendor_address: AddressVendorDetail,
    #[serde(rename = "deliveryOptionId")]
    pub delivery_option_id: i32,
    #[serde(rename = "originalDeliveryOptionFee")]
    pub original_delivery_option_fee: Decimal,
    #[serde(rename = "deliveryOptionFee")]
    pub delivery_option_fee: Decimal,
    #[serde(rename = "userAccountId")]
    pub user_account_id: i32,
    #[serde(rename = "userProfileId")]
    pub user_profile_id: i32,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
}
