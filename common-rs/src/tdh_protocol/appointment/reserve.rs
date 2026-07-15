use crate::tdh_protocol::common::PartialUserIdentity;
use crate::tdh_protocol::consultation::{BookingType, ConsultationChannel};
use crate::tdh_protocol::doctor::profile::DoctorProfile;
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeslot {
    pub start: i64,
    pub end: i64,
    pub duration: i64,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatientPrescreen {
    pub symptom: String,
    pub symptom_duration: i32,
    pub symptom_duration_unit: String,
    pub attachments: Vec<String>,
    pub allergies: Vec<String>,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReserveRequest {
    pub doctor_id: i32,
    pub biz_unit_id: i32,
    pub biz_center_id: i32,
    pub patient_intake: PatientPrescreen,
    pub consultation_channel: ConsultationChannel,
    pub timeslot: Timeslot,
    pub booking_type: BookingType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReserveResponse {
    #[serde(rename = "ReserveResponse.Success")]
    Success(ReserveSuccess),
    #[serde(rename = "ReserveResponse.DoctorNotAvailable")]
    DoctorNotAvailable,
    #[serde(rename = "ReserveResponse.SlotAlreadyBooked")]
    SlotAlreadyBooked,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReserveSuccess {
    pub reserve_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReserveTokenPayload {
    pub booking_id: i64,
    pub doctor_profile: DoctorProfile,
    pub biz_unit_id: i32,
    pub biz_center_id: Option<i32>,
    pub consultation_channel: ConsultationChannel,
    pub patient_identity: PartialUserIdentity,
    pub booking_type: BookingType,
    pub reserved_at: i64,
    pub reserved_until: i64,
}
