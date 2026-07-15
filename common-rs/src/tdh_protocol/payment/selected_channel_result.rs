use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::payment_channel_result::PaymentChannelResult;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "__type")]
pub enum SelectedChannelResult {
    #[serde(rename = "SelectedChannelResult.CoverageChannel")]
    CoverageChannel { channel: Box<PaymentChannelResult> },
    #[serde(rename = "SelectedChannelResult.SelfPayChannel")]
    SelfPayChannel { channel: Box<PaymentChannelResult> },
    #[serde(rename = "SelectedChannelResult.CoverageAndSelfPayChannel")]
    CoverageAndSelfPayChannel {
        #[serde(rename = "coverageChannel")]
        coverage_channel: Box<PaymentChannelResult>,
        #[serde(rename = "selfPayChannel")]
        self_pay_channel: Box<PaymentChannelResult>,
    },
}

impl SelectedChannelResult {
    pub fn grand_total(&self) -> Decimal {
        match self {
            Self::CoverageChannel { channel } => channel.covered_amount().unwrap_or_default(),
            Self::SelfPayChannel { channel } => channel.amount().unwrap_or_default(),
            Self::CoverageAndSelfPayChannel {
                coverage_channel,
                self_pay_channel,
            } => {
                coverage_channel.covered_amount().unwrap_or_default()
                    + self_pay_channel.amount().unwrap_or_default()
            }
        }
    }
}
