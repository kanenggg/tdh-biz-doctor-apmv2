use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::common::tdh_protocol::consultation::ConsultationChannel;
use crate::consultation_config::model::default_timezone;

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct AvailableTimeslotQuery {
    pub doctor_account_id: i64,
    pub doctor_profile_id: i64,
    /// Inclusive start of the query window, RFC-3339.
    #[serde(rename = "from_datetime", alias = "fromDatetime")]
    pub from_datetime: String,
    /// Exclusive end of the query window, RFC-3339.
    #[serde(rename = "to_datetime", alias = "toDatetime")]
    pub to_datetime: String,
    pub consultation_channel: Option<ConsultationChannel>,
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AvailableTimeslotResponse {
    pub doctor_id: Uuid,
    pub timeslots: Vec<AvailableTimeslot>,
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AvailableTimeslot {
    pub timeslot_id: String,
    pub start: String,
    pub end: String,
    pub start_epoch: i64,
    pub end_epoch: i64,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct AvailableDoctorsQuery {
    /// Local calendar date to search, interpreted in `timezone` as [00:00, next day 00:00).
    pub date: String,
    /// IANA timezone for interpreting `date`; defaults to Asia/Bangkok.
    #[serde(default = "default_timezone")]
    pub timezone: String,
    pub consultation_channel: Option<ConsultationChannel>,
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AvailableDoctorsResponse {
    pub date: String,
    pub timezone: String,
    pub doctors: Vec<AvailableDoctor>,
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AvailableDoctor {
    pub doctor_id: Uuid,
    pub doctor_account_id: i64,
    pub doctor_profile_id: i64,
    pub available_timeslot_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_available_timeslot_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledAvailabilityQuery {
    #[serde(rename = "doctor-id", alias = "doctorId", alias = "doctor_id")]
    #[param(rename = "doctor-id")]
    pub doctor_id: Uuid,
    /// Local calendar date to search, interpreted in `timezone` as [00:00, next day 00:00).
    pub date: String,
    /// IANA timezone for interpreting `date`; defaults to Asia/Bangkok.
    #[serde(default = "default_timezone")]
    pub timezone: String,
    pub consultation_channel: Option<ConsultationChannel>,
}

#[cfg(test)]
mod tests {
    use axum::{extract::Query, http::Uri};

    use super::*;

    fn parse_query(query: &str) -> AvailableTimeslotQuery {
        let uri = format!("/v2/doctor-timeslot/available-timeslots?{query}")
            .parse::<Uri>()
            .expect("test URI should parse");
        Query::<AvailableTimeslotQuery>::try_from_uri(&uri)
            .expect("query should deserialize")
            .0
    }

    fn parse_scheduled_query(query: &str) -> ScheduledAvailabilityQuery {
        let uri = format!("/v2/doctor-timeslot/scheduled-availability?{query}")
            .parse::<Uri>()
            .expect("test URI should parse");
        Query::<ScheduledAvailabilityQuery>::try_from_uri(&uri)
            .expect("query should deserialize")
            .0
    }

    #[test]
    fn available_timeslot_query_accepts_camel_case_datetime_params() {
        let query = parse_query(
            "doctorAccountId=42&doctorProfileId=84&fromDatetime=2026-06-17T17%3A00%3A00Z&toDatetime=2026-06-18T17%3A00%3A00Z",
        );

        assert_eq!(query.doctor_account_id, 42);
        assert_eq!(query.doctor_profile_id, 84);
        assert_eq!(query.from_datetime, "2026-06-17T17:00:00Z");
        assert_eq!(query.to_datetime, "2026-06-18T17:00:00Z");
    }

    #[test]
    fn available_timeslot_query_accepts_snake_case_datetime_params() {
        let query = parse_query(
            "doctorAccountId=42&doctorProfileId=84&from_datetime=2026-06-17T17%3A00%3A00Z&to_datetime=2026-06-18T17%3A00%3A00Z",
        );

        assert_eq!(query.from_datetime, "2026-06-17T17:00:00Z");
        assert_eq!(query.to_datetime, "2026-06-18T17:00:00Z");
    }

    #[test]
    fn scheduled_availability_query_accepts_kebab_case_doctor_id_uuid() {
        let doctor_id = uuid::uuid!("018f1414-5e0e-7c2a-b908-7b1967f2b401");
        let query = parse_scheduled_query(
            "doctor-id=018f1414-5e0e-7c2a-b908-7b1967f2b401&date=2026-06-18&timezone=Asia%2FBangkok",
        );

        assert_eq!(query.doctor_id, doctor_id);
        assert_eq!(query.date, "2026-06-18");
        assert_eq!(query.timezone, "Asia/Bangkok");
    }

    #[test]
    fn scheduled_availability_query_keeps_camel_case_doctor_id_compatibility() {
        let doctor_id = uuid::uuid!("018f1414-5e0e-7c2a-b908-7b1967f2b401");
        let query =
            parse_scheduled_query("doctorId=018f1414-5e0e-7c2a-b908-7b1967f2b401&date=2026-06-18");

        assert_eq!(query.doctor_id, doctor_id);
        assert_eq!(query.date, "2026-06-18");
        assert_eq!(query.timezone, "Asia/Bangkok");
    }

    #[test]
    fn available_timeslot_response_omits_consultation_channel() {
        let value = serde_json::to_value(AvailableTimeslot {
            timeslot_id: "018f1414-5e0e-7c2a-b908-7b1967f2b401:1781762400:1781764200:video"
                .to_string(),
            start: "2026-06-18T06:00:00Z".to_string(),
            end: "2026-06-18T06:30:00Z".to_string(),
            start_epoch: 1_781_762_400,
            end_epoch: 1_781_764_200,
        })
        .expect("available timeslot should serialize");

        assert!(value.get("consultationChannel").is_none());
    }
}
