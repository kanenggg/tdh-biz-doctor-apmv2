use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::extend_data::ExtendData;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentSummary {
    pub header: PaymentHeader,
    pub request: PaymentSummaryRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentHeader {
    #[serde(rename = "bizUnitId")]
    pub biz_unit_id: i32,
    #[serde(rename = "bizCenterId")]
    pub biz_center_id: i32,
    #[serde(rename = "flowId")]
    pub flow_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PaymentSummaryRequest {
    #[serde(rename = "moduleId")]
    pub module_id: Option<i32>,
    #[serde(rename = "refCode")]
    pub ref_code: String,
    pub amount: Decimal,
    pub currency: String,
    #[serde(rename = "pricePlanId")]
    pub price_plan_id: i32,
    /// Immutable DoctorApp service-config quote coordinate. For versioned events
    /// this is `profileVersion`; for the committed unversioned event it is its
    /// `occurredAt`. The field name remains a wire-compatibility alias.
    #[serde(
        default,
        rename = "consultationConfigVersion",
        alias = "profileVersion"
    )]
    pub consultation_config_version: Option<i64>,
    #[serde(rename = "isRequireDelivery")]
    pub is_require_delivery: bool,
    #[serde(rename = "extendedData")]
    pub extended_data: Option<ExtendData>,
}
