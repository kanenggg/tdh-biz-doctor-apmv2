use crate::tdh_protocol::consultation::ConsultationChannel;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DolphinAppointmentRequest {
    pub ref_code: String,
    pub user_identity: String,
    pub platform_name: String,
    pub consultation_start_time: i64,
    pub consultation_duration: i32,
    pub patient_device_platform: String,
    pub consultation_channel: ConsultationChannel,
}
