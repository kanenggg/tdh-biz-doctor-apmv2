use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "camelCase")]
pub enum ConsultationSessionActionMessage {
    CreateConsultationSessionOnSchedule {
        booking_id: String,
        user_identity: String,
        sent_at: i64,
    },
    CreateConsultationSessionNow {
        booking_id: String,
        user_identity: String,
    },
    EndConsultationOnSchedule {
        booking_id: String,
        user_identity: String,
        sent_at: i64,
    },
    EndConsultationSessionNow {
        booking_id: String,
        user_identity: String,
    },
}
