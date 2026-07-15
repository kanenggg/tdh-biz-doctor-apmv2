use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookingToken {
    booking_id: i64,
    booking_date_time: i64,
    booking_duration: Duration,
    appointment_start_time: i64,
    consultation_duration: i64,
    buffer_duration: i64,
}
