use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "__type")]
pub enum InsuranceDiscount {
    Percentage { value: i32, amount: Decimal },
    Amount { amount: Decimal },
}
