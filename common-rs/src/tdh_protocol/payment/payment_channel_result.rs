use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::coupon::CouponProtocol;
use super::insurance_discount::InsuranceDiscount;
use super::localized::Localized;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "__type")]
pub enum PaymentChannelResult {
    #[serde(rename = "PaymentChannelResult.Insurance")]
    Insurance {
        #[serde(rename = "insurerCode")]
        insurer_code: String,
        #[serde(rename = "policyId")]
        policy_id: i32,
        #[serde(rename = "coveredAmount")]
        covered_amount: Decimal,
        #[serde(rename = "claimAmount")]
        claim_amount: Decimal,
        discount: Option<InsuranceDiscount>,
        #[serde(rename = "policyPackageType")]
        policy_package_type: Option<String>,
        #[serde(rename = "quotaCoveredAmount")]
        quota_covered_amount: Decimal,
        #[serde(rename = "channelId")]
        channel_id: i32,
        #[serde(rename = "insuranceNameI18n")]
        insurance_name_i18n: Option<Localized>,
        #[serde(rename = "insuranceTypeNameI18n")]
        insurance_type_name_i18n: Option<Localized>,
    },
    #[serde(rename = "PaymentChannelResult.InsuranceV2")]
    InsuranceV2 {
        #[serde(rename = "insurerCode")]
        insurer_code: String,
        #[serde(rename = "bindingId")]
        binding_id: i32,
        #[serde(rename = "coveredAmount")]
        covered_amount: Decimal,
        #[serde(rename = "claimAmount")]
        claim_amount: Decimal,
        discount: Option<InsuranceDiscount>,
        #[serde(rename = "policyPackageType")]
        policy_package_type: Option<String>,
        #[serde(rename = "quotaCoveredAmount")]
        quota_covered_amount: Decimal,
        #[serde(rename = "channelId")]
        channel_id: i32,
        #[serde(rename = "insuranceNameI18n")]
        insurance_name_i18n: Option<Localized>,
        #[serde(rename = "insuranceTypeNameI18n")]
        insurance_type_name_i18n: Option<Localized>,
    },
    #[serde(rename = "PaymentChannelResult.InsuranceV3")]
    InsuranceV3 {
        #[serde(rename = "channelId")]
        channel_id: i32,
        #[serde(rename = "coveredAmount")]
        covered_amount: Decimal,
        #[serde(rename = "claimAmount")]
        claim_amount: Decimal,
        #[serde(rename = "quotaCoveredAmount")]
        quota_covered_amount: Decimal,
        discount: Option<InsuranceDiscount>,
        #[serde(rename = "bindingId")]
        binding_id: i64,
        #[serde(rename = "privilegeId")]
        privilege_id: i32,
        #[serde(rename = "privilegeFlowId")]
        privilege_flow_id: i32,
        #[serde(rename = "providerId")]
        provider_id: i32,
        #[serde(rename = "providerName")]
        provider_name: String,
        #[serde(rename = "providerAbbreviation")]
        provider_abbreviation: String,
        #[serde(rename = "packageTypeName")]
        package_type_name: Option<String>,
        #[serde(rename = "claim3rdRef")]
        claim_3rd_ref: Option<String>,
        #[serde(rename = "insuranceNameI18n")]
        insurance_name_i18n: Option<Localized>,
        #[serde(rename = "insuranceTypeNameI18n")]
        insurance_type_name_i18n: Option<Localized>,
    },
    #[serde(rename = "PaymentChannelResult.EmployeeBenefit")]
    EmployeeBenefit {
        #[serde(rename = "companyId")]
        company_id: i32,
        #[serde(rename = "companyName")]
        company_name: String,
        #[serde(rename = "benefitId")]
        benefit_id: i32,
        #[serde(rename = "benefitCode")]
        benefit_code: String,
        #[serde(rename = "privilegeType")]
        privilege_type: String,
        #[serde(rename = "coveredAmount")]
        covered_amount: Decimal,
        #[serde(rename = "claimAmount")]
        claim_amount: Decimal,
        #[serde(rename = "quotaCoveredAmount")]
        quota_covered_amount: Decimal,
        #[serde(rename = "channelId")]
        channel_id: i32,
        #[serde(rename = "benefitNameI18n")]
        benefit_name_i18n: Option<Localized>,
    },
    #[serde(rename = "PaymentChannelResult.EmployeeBenefitV2")]
    EmployeeBenefitV2 {
        #[serde(rename = "companyCode")]
        company_code: String,
        #[serde(rename = "bindingId")]
        binding_id: i32,
        #[serde(rename = "privilegeType")]
        privilege_type: String,
        #[serde(rename = "coveredAmount")]
        covered_amount: Decimal,
        #[serde(rename = "claimAmount")]
        claim_amount: Decimal,
        #[serde(rename = "quotaCoveredAmount")]
        quota_covered_amount: Decimal,
        #[serde(rename = "customData")]
        custom_data: Option<Value>,
        #[serde(rename = "channelId")]
        channel_id: i32,
        #[serde(rename = "benefitNameI18n")]
        benefit_name_i18n: Option<Localized>,
    },
    #[serde(rename = "PaymentChannelResult.CampaignLocation")]
    CampaignLocation {
        #[serde(rename = "campaignId")]
        campaign_id: i32,
        latitude: String,
        longitude: String,
        #[serde(rename = "coveredAmount")]
        covered_amount: Decimal,
        #[serde(rename = "claimAmount")]
        claim_amount: Decimal,
        coupon: CouponProtocol,
        #[serde(rename = "channelId")]
        channel_id: i32,
    },
    #[serde(rename = "PaymentChannelResult.Campaign")]
    Campaign {
        #[serde(rename = "campaignId")]
        campaign_id: i32,
        #[serde(rename = "coveredAmount")]
        covered_amount: Decimal,
        #[serde(rename = "claimAmount")]
        claim_amount: Decimal,
        coupon: CouponProtocol,
        #[serde(rename = "channelId")]
        channel_id: i32,
    },
    #[serde(rename = "PaymentChannelResult.Card")]
    Card { id: String, amount: Decimal },
    #[serde(rename = "PaymentChannelResult.Wallet")]
    Wallet { amount: Decimal },
    #[serde(rename = "PaymentChannelResult.PromptPay")]
    PromptPay { id: String, amount: Decimal },
    #[serde(rename = "PaymentChannelResult.TrueMoney")]
    TrueMoney { id: String, amount: Decimal },
}

impl PaymentChannelResult {
    pub fn covered_amount(&self) -> Option<Decimal> {
        match self {
            Self::Insurance { covered_amount, .. }
            | Self::InsuranceV2 { covered_amount, .. }
            | Self::InsuranceV3 { covered_amount, .. }
            | Self::EmployeeBenefit { covered_amount, .. }
            | Self::EmployeeBenefitV2 { covered_amount, .. }
            | Self::CampaignLocation { covered_amount, .. }
            | Self::Campaign { covered_amount, .. } => Some(*covered_amount),
            _ => None,
        }
    }

    pub fn amount(&self) -> Option<Decimal> {
        match self {
            Self::Card { amount, .. }
            | Self::Wallet { amount }
            | Self::PromptPay { amount, .. }
            | Self::TrueMoney { amount, .. } => Some(*amount),
            _ => None,
        }
    }

    pub fn channel_id(&self) -> Option<i32> {
        match self {
            Self::Insurance { channel_id, .. }
            | Self::InsuranceV2 { channel_id, .. }
            | Self::InsuranceV3 { channel_id, .. }
            | Self::EmployeeBenefit { channel_id, .. }
            | Self::EmployeeBenefitV2 { channel_id, .. }
            | Self::CampaignLocation { channel_id, .. }
            | Self::Campaign { channel_id, .. } => Some(*channel_id),
            _ => None,
        }
    }

    pub fn is_coverage_channel(&self) -> bool {
        matches!(
            self,
            Self::Insurance { .. }
                | Self::InsuranceV2 { .. }
                | Self::InsuranceV3 { .. }
                | Self::EmployeeBenefit { .. }
                | Self::EmployeeBenefitV2 { .. }
                | Self::CampaignLocation { .. }
                | Self::Campaign { .. }
        )
    }

    pub fn is_self_pay_channel(&self) -> bool {
        matches!(
            self,
            Self::Card { .. }
                | Self::Wallet { .. }
                | Self::PromptPay { .. }
                | Self::TrueMoney { .. }
        )
    }
}
