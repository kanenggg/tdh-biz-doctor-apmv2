use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::event::biz_apm::{identity_model::PartialUserIdentity, model::ConsultationChannel};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
enum FollowUpEvent {
    #[serde(rename = "FollowUpRequired")]
    FollowUpRequired(FollowUpRequiredEvent),
    #[serde(rename = "FollowUpRequestExpired")]
    FollowUpRequestExpired(FollowUpRequestExpiredEvent),
    #[serde(rename = "PatientAcceptedFollowUp")]
    PatientAcceptedFollowUp(PatientAcceptedFollowUpEvent),
    #[serde(rename = "FollowUpCancelled")]
    FollowUpCancelled(FollowUpCancelledEvent),
}

/// Follow up required event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpRequiredEvent {
    pub previous_booking_id: String,
    pub follow_up_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_id: Uuid,
    pub biz_unit_id: i32,
    pub consultation_start_time: i64,
    pub consultation_duration_in_second: i32,
    pub consultation_fee: f64,
    pub consultation_channel: ConsultationChannel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_patient_note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_note: Option<String>,
    pub created_at: i64,
}

/// Follow up request expired event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpRequestExpiredEvent {
    pub previous_booking_id: String,
    pub follow_up_id: String,
    pub doctor_id: Uuid,
    pub patient_identity: PartialUserIdentity,
    pub created_at: i64,
}

/// Follow up cancelled event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpCancelledEvent {
    pub previous_booking_id: String,
    pub follow_up_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_id: Uuid,
    pub created_at: i64,
}

/// Patient accepted follow up event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatientAcceptedFollowUpEvent {
    pub previous_booking_id: String,
    pub follow_up_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_id: Uuid,
    pub consultation_start_time: i64,
    pub consultation_duration_in_second: i32,
    pub consultation_fee: f64,
    pub symptoms: String,
    pub consultation_channel: ConsultationChannel,
    pub created_at: i64,
}
