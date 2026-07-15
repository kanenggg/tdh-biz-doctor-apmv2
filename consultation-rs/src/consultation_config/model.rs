use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TimePeriod {
    pub start_time: i32,
    pub end_time: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DateWithTimePeriods {
    #[schema(value_type = String, format = Date, example = "2026-05-20")]
    pub date: String,
    pub periods: Vec<TimePeriod>,
}

pub fn default_timezone() -> String {
    "Asia/Bangkok".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleAvailableConfig {
    pub specific_date: Vec<DateWithTimePeriods>,
    #[serde(default = "default_timezone")]
    #[schema(value_type = String, example = "Asia/Bangkok")]
    pub timezone: String,
    #[serde(alias = "dayOfWeek")]
    pub days_of_week: BTreeMap<i32, Vec<TimePeriod>>,
}

impl Default for ScheduleAvailableConfig {
    fn default() -> Self {
        Self {
            specific_date: Vec::new(),
            timezone: default_timezone(),
            days_of_week: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
#[serde(tag = "__type")]
pub enum UpdateScheduleConfigResponse {
    Success,
    #[serde(rename = "Failure.ConflictTimeOverlap")]
    ConflictTimeOverlap {
        days: Vec<i32>,
    },
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAvailabilityRequest {
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilityResponse {
    #[serde(rename = "__type")]
    pub response_type: String,
    pub schedule_available: bool,
    pub instant_available: bool,
}

impl AvailabilityResponse {
    pub fn success(schedule_available: bool, instant_available: bool) -> Self {
        Self {
            response_type: "Success".to_string(),
            schedule_available,
            instant_available,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum SuccessResponse {
    Success,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsultationAvailability {
    pub schedule_available: bool,
    pub instant_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DoctorIdentity {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doctor_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doctor_account_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub doctor_profile_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_doctor_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EventSource {
    pub service: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoctorAuthContext {
    pub doctor_account_id: i64,
    pub doctor_profile_id: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoctorConfigIdentity {
    pub doctor_id: Uuid,
    pub doctor_account_id: i64,
    pub doctor_profile_id: i64,
}

impl DoctorConfigIdentity {
    pub fn event_doctor(self) -> DoctorIdentity {
        DoctorIdentity {
            doctor_id: Some(self.doctor_id),
            doctor_account_id: Some(self.doctor_account_id),
            doctor_profile_id: Some(self.doctor_profile_id),
            legacy_doctor_id: None,
        }
    }
}
