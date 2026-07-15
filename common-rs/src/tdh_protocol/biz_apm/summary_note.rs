use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::PatientIdentity;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Icd10 {
    pub code: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DurationUnit {
    pub unit: String,
    pub value: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SummarizationRequest {
    pub booking_id: String,
    pub prescription_id: Option<i64>,
    pub present_illness: String,
    pub chief_complaint: String,
    pub diagnosis: String,
    pub recommendations: String,
    #[serde(rename = "icd10")]
    pub icd10: Vec<Icd10>,
    pub illness_duration: DurationUnit,
    pub note_to_staff: String,
    pub follow_up: super::follow_up::FollowUp,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(tag = "__type")]
pub enum SummarizationResult {
    #[serde(rename = "SummarizationResult.Success", rename_all = "camelCase")]
    Success {
        summary_note_id: i64,
        patient_identity: PatientIdentity,
        biz_unit_id: i64,
        biz_center_id: i64,
    },
    #[serde(
        rename = "SummarizationResult.AlreadySubmitted",
        rename_all = "camelCase"
    )]
    AlreadySubmitted {
        summary_note_id: i64,
        patient_identity: PatientIdentity,
        biz_unit_id: i64,
        biz_center_id: i64,
    },
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum SummarizationError {
    #[error("Internal error: {0}")]
    InternalError(String),
    #[error("Follow-up creation failed: {0}")]
    FollowUpCreationFailed(String),
    #[error("Parent appointment is not fulfilled")]
    ParentNotFulfilled,
}
