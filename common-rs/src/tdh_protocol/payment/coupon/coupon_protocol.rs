use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::discount_v2::DiscountV2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "__type")]
pub enum CouponProtocol {
    #[serde(rename = "CouponProtocol.Coupon")]
    Coupon {
        #[serde(rename = "tenantId")]
        tenant_id: i32,
        #[serde(rename = "bizUnitId")]
        biz_unit_id: i32,
        #[serde(rename = "bizCenterId")]
        biz_center_id: i32,
        #[serde(rename = "moduleId")]
        module_id: Option<i32>,
        #[serde(rename = "userAccountId")]
        user_account_id: i32,
        #[serde(rename = "userProfileId")]
        user_profile_id: i32,
        #[serde(rename = "couponCampaignId")]
        coupon_campaign_id: i32,
        #[serde(rename = "campaignName")]
        campaign_name: String,
        coupon: String,
        discount: DiscountV2,
        currency: String,
        #[serde(rename = "finalDiscountAmount")]
        final_discount_amount: Decimal,
        #[serde(rename = "ackRefKey")]
        ack_ref_key: String,
        #[serde(rename = "campaignConfig")]
        campaign_config: Option<String>,
    },
    #[serde(rename = "CouponProtocol.LegacyCoupon")]
    LegacyCoupon {
        #[serde(rename = "tenantId")]
        tenant_id: i32,
        #[serde(rename = "bizUnitId")]
        biz_unit_id: i32,
        #[serde(rename = "bizCenterId")]
        biz_center_id: i32,
        #[serde(rename = "moduleId")]
        module_id: Option<i32>,
        #[serde(rename = "userAccountId")]
        user_account_id: i32,
        #[serde(rename = "userProfileId")]
        user_profile_id: i32,
        #[serde(rename = "legacyUserId")]
        legacy_user_id: Option<String>,
        #[serde(rename = "legacyCouponId")]
        legacy_coupon_id: i32,
        #[serde(rename = "campaignName")]
        campaign_name: String,
        coupon: String,
        discount: DiscountV2,
        currency: String,
        #[serde(rename = "finalDiscountAmount")]
        final_discount_amount: Decimal,
        #[serde(rename = "ackRefKey")]
        ack_ref_key: String,
        #[serde(rename = "campaignConfig")]
        campaign_config: Option<String>,
    },
}

impl CouponProtocol {
    pub fn tenant_id(&self) -> i32 {
        match self {
            Self::Coupon { tenant_id, .. } => *tenant_id,
            Self::LegacyCoupon { tenant_id, .. } => *tenant_id,
        }
    }

    pub fn biz_unit_id(&self) -> i32 {
        match self {
            Self::Coupon { biz_unit_id, .. } => *biz_unit_id,
            Self::LegacyCoupon { biz_unit_id, .. } => *biz_unit_id,
        }
    }

    pub fn biz_center_id(&self) -> i32 {
        match self {
            Self::Coupon { biz_center_id, .. } => *biz_center_id,
            Self::LegacyCoupon { biz_center_id, .. } => *biz_center_id,
        }
    }

    pub fn module_id(&self) -> Option<i32> {
        match self {
            Self::Coupon { module_id, .. } => *module_id,
            Self::LegacyCoupon { module_id, .. } => *module_id,
        }
    }

    pub fn user_account_id(&self) -> i32 {
        match self {
            Self::Coupon {
                user_account_id, ..
            } => *user_account_id,
            Self::LegacyCoupon {
                user_account_id, ..
            } => *user_account_id,
        }
    }

    pub fn user_profile_id(&self) -> i32 {
        match self {
            Self::Coupon {
                user_profile_id, ..
            } => *user_profile_id,
            Self::LegacyCoupon {
                user_profile_id, ..
            } => *user_profile_id,
        }
    }

    pub fn campaign_name(&self) -> &str {
        match self {
            Self::Coupon { campaign_name, .. } => campaign_name,
            Self::LegacyCoupon { campaign_name, .. } => campaign_name,
        }
    }

    pub fn coupon(&self) -> &str {
        match self {
            Self::Coupon { coupon, .. } => coupon,
            Self::LegacyCoupon { coupon, .. } => coupon,
        }
    }

    pub fn discount(&self) -> &DiscountV2 {
        match self {
            Self::Coupon { discount, .. } => discount,
            Self::LegacyCoupon { discount, .. } => discount,
        }
    }

    pub fn currency(&self) -> &str {
        match self {
            Self::Coupon { currency, .. } => currency,
            Self::LegacyCoupon { currency, .. } => currency,
        }
    }

    pub fn final_discount_amount(&self) -> &Decimal {
        match self {
            Self::Coupon {
                final_discount_amount,
                ..
            } => final_discount_amount,
            Self::LegacyCoupon {
                final_discount_amount,
                ..
            } => final_discount_amount,
        }
    }

    pub fn ack_ref_key(&self) -> &str {
        match self {
            Self::Coupon { ack_ref_key, .. } => ack_ref_key,
            Self::LegacyCoupon { ack_ref_key, .. } => ack_ref_key,
        }
    }

    pub fn campaign_config(&self) -> Option<&str> {
        match self {
            Self::Coupon {
                campaign_config, ..
            } => campaign_config.as_deref(),
            Self::LegacyCoupon {
                campaign_config, ..
            } => campaign_config.as_deref(),
        }
    }
}
