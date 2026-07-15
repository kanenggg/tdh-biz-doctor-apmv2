use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "camelCase")]
pub enum ConsultationBriefStatus {
    UpComingWithoutDoctor,
    UpComingWithDoctor { doctor_id: i32 },
    InProgress { doctor_id: i32 },
    AppointmentSuccess { doctor_id: i32 },
    ConsultationSummaryIsReady { doctor_id: i32 },
    PrescriptionIsReady { doctor_id: i32 },
    Missed { doctor_id: i32 },
    Cancelled { doctor_id: Option<i32> },
}
