use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::lalamove::LalamovePlaceOrderData;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "__type")]
pub enum DeliveryOptionDetail {
    #[serde(rename = "DeliveryOptionDetail.LalamoveDeliveryOption")]
    LalamoveDeliveryOption {
        data: LalamovePlaceOrderData,
        #[serde(rename = "originalDeliveryFee")]
        original_delivery_fee: Decimal,
    },
}
