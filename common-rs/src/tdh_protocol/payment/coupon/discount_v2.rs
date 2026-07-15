use serde::{Deserialize, Serialize};

use super::discount_calculate_cond::DiscountCalculateCond;
use super::discount_condition::DiscountCondition;
use super::discount_type::DiscountType;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiscountV2 {
    #[serde(rename = "discountType")]
    pub discount_type: DiscountType,
    #[serde(rename = "discountCondition")]
    pub discount_condition: Option<DiscountCondition>,
    #[serde(rename = "discountCalculateCond")]
    pub discount_calculate_cond: Option<DiscountCalculateCond>,
}
