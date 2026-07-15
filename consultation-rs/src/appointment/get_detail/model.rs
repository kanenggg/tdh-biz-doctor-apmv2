use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    appointment::types::AppointmentTime,
    repo::enums::{AppointmentStatusEnum, BookingTypeEnum, ConsultationChannelEnum},
};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PartialUserIdentity {
    pub account_id: i32,
    pub profile_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PrescreenInfo {
    pub symptom: String,
    pub duration: i32,
    #[serde(alias = "duration_unit")]
    pub duration_unit: String,
    pub attachments: Vec<String>,
    pub allergies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppointmentDetail {
    pub booking_id: String,
    pub appointment_time: AppointmentTime,
    pub status: AppointmentStatusEnum,
    pub booking_type: BookingTypeEnum,
    pub consultation_channel: ConsultationChannelEnum,
    pub patient: PartialUserIdentity,
    pub doctor: PartialUserIdentity,
    pub prescreen: PrescreenInfo,
    pub payment_tx_id: i64,
    pub payment_tx_ref_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
pub enum GetAppointmentDetailResponse {
    Success(AppointmentDetail),
    AppointmentNotFound,
}
