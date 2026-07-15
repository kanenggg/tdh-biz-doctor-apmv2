use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "__type")]
pub enum DiscountCondBase {
    #[serde(rename = "DiscountType.LowestSpending")]
    LowestSpending {
        value: Decimal,
        #[serde(rename = "isMust")]
        is_must: bool,
    },
    #[serde(rename = "DiscountType.LowestQuantity")]
    LowestQuantity {
        qty: i32,
        #[serde(rename = "isMust")]
        is_must: bool,
    },
}

impl DiscountCondBase {
    pub fn is_must(&self) -> bool {
        match self {
            Self::LowestSpending { is_must, .. } => *is_must,
            Self::LowestQuantity { is_must, .. } => *is_must,
        }
    }
}
