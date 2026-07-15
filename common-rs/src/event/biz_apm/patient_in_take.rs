use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Nhso {
    pub symptom_id: i32,
    pub symptom_type: String,
    pub given_detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MordeeApp {
    pub symptom: String,
    pub duration: i32,
    #[serde(alias = "duration_unit")]
    pub duration_unit: String,
    pub attachments: Vec<String>,
    pub allergies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum PatientInTakeData {
    MordeeApp(MordeeApp),
    Nhso(Nhso),
}
