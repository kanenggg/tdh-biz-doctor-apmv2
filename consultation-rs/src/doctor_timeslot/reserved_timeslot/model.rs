use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReserveTimeSlot {
    /// Public Booking ID projected from the canonical Hold or Appointment occupancy owner.
    pub booking_id: String,
    /// appointment_start as epoch seconds, UTC.
    pub start_time: i64,
    /// appointment_end as epoch seconds, UTC.
    pub end_time: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReservedTimeslotsResponse {
    pub reserved_timeslots: Vec<ReserveTimeSlot>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ReservedTimeslotsQuery {
    pub doctor_profile_id: i32,
    /// Inclusive start of the query window, RFC-3339 (e.g. 2026-06-17T17:00:00Z).
    #[serde(rename = "from_datetime", alias = "fromDatetime")]
    pub from_datetime: String,
    /// Exclusive end of the query window, RFC-3339 (e.g. 2026-06-18T17:00:00Z).
    #[serde(rename = "to_datetime", alias = "toDatetime")]
    pub to_datetime: String,
}

#[cfg(test)]
mod tests {
    use axum::{extract::Query, http::Uri};

    use super::*;

    fn parse_query(query: &str) -> ReservedTimeslotsQuery {
        let uri = format!("/v2/internal/doctor-timeslot/reserved-timeslots?{query}")
            .parse::<Uri>()
            .expect("test URI should parse");
        Query::<ReservedTimeslotsQuery>::try_from_uri(&uri)
            .expect("query should deserialize")
            .0
    }

    #[test]
    fn reserved_timeslot_query_accepts_camel_case_datetime_params() {
        let query = parse_query(
            "doctorProfileId=84&fromDatetime=2026-06-17T17%3A00%3A00Z&toDatetime=2026-06-18T17%3A00%3A00Z",
        );

        assert_eq!(query.doctor_profile_id, 84);
        assert_eq!(query.from_datetime, "2026-06-17T17:00:00Z");
        assert_eq!(query.to_datetime, "2026-06-18T17:00:00Z");
    }

    #[test]
    fn reserved_timeslot_query_accepts_snake_case_datetime_params() {
        let query = parse_query(
            "doctorProfileId=84&from_datetime=2026-06-17T17%3A00%3A00Z&to_datetime=2026-06-18T17%3A00%3A00Z",
        );

        assert_eq!(query.from_datetime, "2026-06-17T17:00:00Z");
        assert_eq!(query.to_datetime, "2026-06-18T17:00:00Z");
    }
}
