use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Localized {
    #[serde(rename = "th-TH")]
    pub th: String,
    #[serde(rename = "en-US")]
    pub en: String,
}
