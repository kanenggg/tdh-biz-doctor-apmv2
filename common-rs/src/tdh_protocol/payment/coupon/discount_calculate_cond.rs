use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscountCalculateCond {
    #[serde(rename = "maxQty")]
    pub max_qty: Option<i32>,
    #[serde(rename = "limitAmount")]
    pub limit_amount: Option<Decimal>,
}
