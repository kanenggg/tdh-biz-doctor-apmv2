use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatientPrescreen {
    pub symptom: String,
    pub symptom_duration: String,
    pub symptom_duration_unit: String,
    pub attachments: Vec<String>,
    pub allergies: Vec<String>,
}
