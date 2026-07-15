use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "camelCase")]
pub enum NotifyDoctorActionMessage {
    InstantNotifyDoctorActionMessage {
        booking_id: String,
    },
    ScheduledNotifyDoctorActionMessage {
        booking_id: String,
        scheduled_time: i64,
    },
}
