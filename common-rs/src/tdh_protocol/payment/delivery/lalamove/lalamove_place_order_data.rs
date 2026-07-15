use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LalamovePlaceOrderData {
    #[serde(rename = "quotationId")]
    pub quotation_id: String,
    pub sender: Sender,
    pub recipients: Vec<DeliveryDetails>,
    #[serde(rename = "isPODEnabled")]
    pub is_pod_enabled: Option<bool>,
    pub partner: Option<bool>,
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeliveryDetails {
    #[serde(rename = "stopId")]
    pub stop_id: String,
    pub name: String,
    pub phone: String,
    pub address: Option<String>,
    pub remarks: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Sender {
    #[serde(rename = "stopId")]
    pub stop_id: String,
    pub name: String,
    pub phone: String,
    pub address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Metadata {
    #[serde(rename = "ackRefKey")]
    pub ack_ref_key: String,
}
