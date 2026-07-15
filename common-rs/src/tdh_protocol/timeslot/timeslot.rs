use jiff::civil::Date;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DoctorTimeslot {
    pub slot_id: i64,
    pub slot_date: Date,
    pub start_time: jiff::civil::Time,
    pub end_time: jiff::civil::Time,
}
