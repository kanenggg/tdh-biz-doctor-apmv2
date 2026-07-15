use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum DeliveryOptionExtendDiscount {
    Coverage(Coverage),
    SelfPay(SelfPay),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "__type")]
pub enum Coverage {
    #[serde(rename = "DeliveryOptionExtendDiscount.Coverage.Insurance")]
    Insurance {
        #[serde(rename = "channelId")]
        channel_id: i32,
        #[serde(rename = "allowedSendCoverage")]
        allowed_send_coverage: bool,
        #[serde(rename = "insurerCode")]
        insurer_code: String,
        #[serde(rename = "discountAmount")]
        discount_amount: Decimal,
    },
    #[serde(rename = "DeliveryOptionExtendDiscount.Coverage.EmployeeBenefit")]
    EmployeeBenefit {
        #[serde(rename = "channelId")]
        channel_id: i32,
        #[serde(rename = "allowedSendCoverage")]
        allowed_send_coverage: bool,
        #[serde(rename = "privilegeType")]
        privilege_type: String,
        #[serde(rename = "discountAmount")]
        discount_amount: Decimal,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "__type")]
pub enum SelfPay {
    #[serde(rename = "DeliveryOptionExtendDiscount.SelfPay.Card")]
    Card {
        #[serde(rename = "discountAmount")]
        discount_amount: Decimal,
    },
    #[serde(rename = "DeliveryOptionExtendDiscount.SelfPay.PromptPay")]
    PromptPay {
        #[serde(rename = "discountAmount")]
        discount_amount: Decimal,
    },
    #[serde(rename = "DeliveryOptionExtendDiscount.SelfPay.TrueMoney")]
    TrueMoney {
        #[serde(rename = "discountAmount")]
        discount_amount: Decimal,
    },
}
