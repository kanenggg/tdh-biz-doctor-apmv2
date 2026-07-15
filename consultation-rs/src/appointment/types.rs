use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppointmentTime {
    pub start_time: i64,
    pub end_time: i64,
}
