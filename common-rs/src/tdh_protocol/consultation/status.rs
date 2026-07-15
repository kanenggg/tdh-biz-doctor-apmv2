use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "camelCase")]
pub enum ConsultationStatus {
    UpComingWithoutDoctor,
    UpComingWithDoctor {
        doctor_id: i32,
    },
    InProgress {
        doctor_id: i32,
    },
    AppointmentSuccess {
        doctor_id: i32,
    },
    ConsultationSummaryIsReady {
        doctor_id: i32,
        diagnosis: String,
    },
    PrescriptionIsReady {
        doctor_id: i32,
        diagnosis: String,
        prescription_id: String,
    },
    Missed {
        doctor_id: i32,
    },
    Cancelled {
        doctor_id: Option<i32>,
    },
}
