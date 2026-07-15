use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, Debug, Serialize)]
pub struct DrugAllergy {
    pub id: i32,
    pub description: String,
}

#[derive(Deserialize, Debug, Serialize)]
#[serde(tag = "__type")]
pub enum DrugAllergyInfo {
    #[serde(rename = "HasDrugAllergies")]
    #[serde(rename_all = "camelCase")]
    HasDrugAllergies {
        drug_allergies: Option<Vec<DrugAllergy>>,
    },
    #[serde(rename = "NoDrugAllergies")]
    NoDrugAllergies,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Precription {
    pub medicine: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrescriptionRequest {
    pub booking_id: String,
    pub biz_unit_id: i32,
    pub biz_center_id: i32,
    pub doctor_id: i32,
    pub patient_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prescription_no: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prescription_expired_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acknowlege_allergy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allergies: Option<Vec<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub items: Option<Vec<PrescriptionItemRequest>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrescriptionItemRequest {
    pub medicine_id: i32,
    pub price_plan_id: i32,
    pub dosage: i32,
    pub dosage_id: i32,
    pub route_id: i32,
    pub frequency_id: i32,
    pub duration: i32,
    pub duration_id: i32,
    pub indication_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patient_detail: Option<String>,
    pub unit_amount: i32,
    pub unit_cost: f64,
    pub total_amount: f64,
}
