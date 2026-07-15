use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::appointment::types::AppointmentTime;

/// Query filter for the appointment list. `patient_account_id` is required —
/// axum rejects a missing/unparseable value with 400 before the handler runs.
/// `patient_profile_id` is optional and narrows the result to one profile.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAppointmentsQuery {
    pub patient_account_id: i32,
    #[serde(default)]
    pub patient_profile_id: Option<i32>,
}

/// Doctor on a listed appointment: identity (account/profile) plus a name snapshot.
///
/// TODO: the first/last name snapshot is NOT yet persisted on the appointment
/// record. Until the write path snapshots it, the service mocks the name (see
/// `service::mock_doctor`). The IDs are real; the names are placeholders.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppointmentDoctor {
    pub account_id: i32,
    pub profile_id: i32,
    pub first_name: String,
    pub last_name: String,
}

/// One fulfilled appointment in the list — the appointment envelope only.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppointmentListItem {
    pub booking_id: String,
    pub appointment_time: AppointmentTime,
    pub doctor: AppointmentDoctor,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppointmentList {
    pub appointments: Vec<AppointmentListItem>,
}

/// Read response. 200-always tagged enum, mirroring `get_detail`/`consultation_summary`.
/// Only `Success` exists today; an empty result is `Success` with an empty list.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
pub enum ListAppointmentsResponse {
    Success(AppointmentList),
}
