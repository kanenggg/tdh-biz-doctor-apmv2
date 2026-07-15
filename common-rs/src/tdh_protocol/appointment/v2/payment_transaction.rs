pub type PaymentChannels = Vec<PaymentChannel>;

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "__type")]
pub enum PaymentChannel {
    #[serde(rename = "Insurance")]
    Insurance { binding_id: i64, privilege_id: i32 },
    #[serde(rename = "EmployeeBenefit")]
    EmployeeBenefit {},
    #[serde(rename = "CampaignLocation")]
    CampaignLocation {},
    #[serde(rename = "Campaign")]
    Campaign,
    #[serde(rename = "Card")]
    Card { id: String },
    #[serde(rename = "PromptPay")]
    PromptPay { id: String },
    #[serde(rename = "TrueMoney")]
    TrueMoney { id: String },
}
