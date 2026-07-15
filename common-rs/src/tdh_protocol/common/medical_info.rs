use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MedicalInfo {
    pub symptoms: String,
    pub duration: i32,
    pub duration_uo_m: String,
    pub congenital_diseases: String,
    pub drug_allergies: String,
    pub image_urls: Vec<Url>,
}
