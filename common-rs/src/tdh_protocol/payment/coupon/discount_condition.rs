use serde::{Deserialize, Serialize};

use super::discount_cond_base::DiscountCondBase;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscountCondition {
    #[serde(rename = "lowestSpending")]
    pub lowest_spending: Option<DiscountCondBase>,
    #[serde(rename = "lowestQuantity")]
    pub lowest_quantity: Option<DiscountCondBase>,
}
