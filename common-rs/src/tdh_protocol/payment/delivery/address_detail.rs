use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AddressDetail {
    #[serde(rename = "addressId")]
    pub address_id: i32,
    pub name: String,
    #[serde(rename = "contactName")]
    pub contact_name: String,
    #[serde(rename = "addressInfo")]
    pub address_info: String,
    pub latitude: f64,
    pub longitude: f64,
    #[serde(rename = "telephoneNumber")]
    pub telephone_number: String,
    pub note: Option<String>,
}
