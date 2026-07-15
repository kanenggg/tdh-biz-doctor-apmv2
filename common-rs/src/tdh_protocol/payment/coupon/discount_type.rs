use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "__type")]
pub enum DiscountType {
    #[serde(rename = "DiscountType.Percentage")]
    Percentage { value: Decimal },
    #[serde(rename = "DiscountType.Amount")]
    Amount { value: Decimal },
    #[serde(rename = "DiscountType.QtyAmount")]
    QtyAmount {
        value: i32,
        #[serde(rename = "matchQty")]
        match_qty: i32,
    },
    #[serde(rename = "DiscountType.QuotaAmount")]
    QuotaAmount { value: Decimal },
}
