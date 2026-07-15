use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::appointment::types::AppointmentTime;
use crate::protocol::follow_up::FollowUp;
use crate::protocol::summary_note::{DrugAllergy, DurationUnit, Icd10};
use crate::repo::enums::ConsultationChannelEnum;

/// Doctor reference for a consultation. IDs only — name/specialties are owned elsewhere
/// and resolved by the BFF, not this core service.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorRef {
    pub doctor_id: i32,
    pub doctor_account_id: i32,
    pub doctor_profile_id: i32,
}

/// The clinical content of a fulfilled appointment, decrypted from `doctor_summary_note`.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationSummaryNote {
    /// Reference only; prescription items are out of scope for this service.
    pub prescription_id: Option<i64>,
    pub present_illness: String,
    pub chief_complaint: String,
    pub diagnosis: String,
    pub recommendations: String,
    #[serde(rename = "icd10")]
    pub icd10: Vec<Icd10>,
    pub illness_duration: DurationUnit,
    pub note_to_staff: String,
    pub drug_allergies: Option<Vec<DrugAllergy>>,
}

/// Canonical consultation-summary detail for a fulfilled appointment.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationSummary {
    pub booking_id: String,
    pub appointment_time: AppointmentTime,
    pub consultation_channel: ConsultationChannelEnum,
    pub doctor: DoctorRef,
    pub summary_note: ConsultationSummaryNote,
    pub follow_up: FollowUp,
}

/// Read response. 200-always tagged enum, mirroring `get_detail`.
///
/// - `NotFound`     — booking does not exist, or it is FULFILLED but has no summary note.
/// - `NotFulfilled` — booking exists but its appointment status is not FULFILLED.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
pub enum ConsultationSummaryResponse {
    Success(ConsultationSummary),
    NotFound,
    NotFulfilled,
}
