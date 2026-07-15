use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppointmentStatus {
    Proposed,
    Pending,
    Booked,
    Arrived,
    Fulfilled,
    Cancelled,
    Noshow,
    #[serde(rename = "ENTERED_IN_ERROR")]
    EnteredInError,
}
