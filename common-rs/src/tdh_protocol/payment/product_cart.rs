use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProductCart {
    #[serde(rename = "productId")]
    pub product_id: i32,
    #[serde(rename = "pricePlanId")]
    pub price_plan_id: i32,
    #[serde(rename = "finalPrice")]
    pub final_price: Decimal,
    pub quantity: i32,
}
