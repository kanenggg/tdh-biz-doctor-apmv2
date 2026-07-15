use crate::tdh_protocol::common::PartialUserIdentity;
use crate::tdh_protocol::consultation::ConsultationChannel;
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type")]
pub enum BookingType {
    #[serde(rename = "Instant")]
    Instant,
    #[serde(rename = "Schedule")]
    Schedule,
}

impl From<BookingType> for String {
    fn from(val: BookingType) -> Self {
        match val {
            BookingType::Instant => "Instant".to_string(),
            BookingType::Schedule => "Schedule".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type")]
pub enum PreSessionMessage {
    #[serde(rename_all = "camelCase")]
    TimeslotReserved {
        booking_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        biz_unit_id: i32,
        reserved_from: i64,
        reservation_duration_sec: i64,
        consultation_channel: ConsultationChannel,
        reserved_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    ReservationCancelled {
        booking_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        biz_unit_id: i32,
        cancelled_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    ReservationExpired {
        booking_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        biz_unit_id: i32,
        cancelled_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    ConsultationBooked {
        booking_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        doctor_name: String,
        doctor_specialties: Vec<String>,
        biz_unit_id: i32,
        payment_module_id: i32,
        booking_type: BookingType,
        consultation_start_time: i64,
        consultation_duration_in_second: i32,
        consultation_fee: f64,
        symptoms: String,
        consultation_channel: ConsultationChannel,
        booked_at: i64,
        doctor_image_url: String,
    },
    #[serde(rename_all = "camelCase")]
    ConsultationCancelled {
        booking_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        biz_unit_id: i32,
        payment_module_id: i32,
        cancel_code: String,
        cancelled_at: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type", rename_all = "camelCase")]
pub enum SessionParticipant {
    Patient,
    Doctor,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type")]
pub enum TerminationCode {
    #[serde(rename_all = "camelCase")]
    SuccessfulSession {
        patient_joined_at: i64,
        doctor_joined_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    PatientAbsent {
        doctor_joined_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    DoctorAbsent {
        patient_joined_at: i64,
    },
    BothPartiesAbsent,
    #[serde(rename_all = "camelCase")]
    TechnicalError {
        error_message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type")]
pub enum SessionMessage {
    #[serde(rename_all = "camelCase")]
    SessionCreated {
        booking_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        session_provider: String,
        session_initiator: SessionParticipant,
        consultation_start_time: i64,
        consultation_duration_in_second: i32,
        created_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    PatientJoined {
        booking_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        joined_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    DoctorJoined {
        booking_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        joined_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    AllParticipantJoined {
        booking_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        patient_joined_at: i64,
        doctor_joined_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    PatientDisconnected {
        booking_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        disconnected_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    DoctorDisconnected {
        booking_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        disconnected_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    SessionTerminated {
        booking_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        termination_code: TerminationCode,
        terminated_by: SessionParticipant,
        terminated_at: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Medicine {
    pub price_plan_id: i32,
    pub medicine_id: i32,
    pub medicine_name_en: String,
    pub medicine_name_th: String,
    pub medicine_instruction_en: String,
    pub medicine_instruction_th: String,
    pub medicine_image_url: String,
    pub medicine_amount: i32,
    pub medicine_unit_price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrescriptionInfo {
    pub prescription_refcode: String,
    pub medicine_items: Vec<Medicine>,
    pub expire_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "__type")]
pub enum PostSessionMessage {
    #[serde(rename_all = "camelCase")]
    ConsultationSummarized {
        booking_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        doctor_note: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        prescription_info: Option<PrescriptionInfo>,
        summarized_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    FollowUpRequired {
        previous_booking_id: String,
        follow_up_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        biz_unit_id: i32,
        consultation_start_time: i64,
        consultation_duration_in_second: i32,
        consultation_fee: f64,
        consultation_channel: ConsultationChannel,
        #[serde(skip_serializing_if = "Option::is_none")]
        additional_patient_note: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        internal_note: Option<String>,
        created_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    FollowUpRequestExpired {
        previous_booking_id: String,
        follow_up_id: String,
        doctor_id: i32,
        patient_identity: PartialUserIdentity,
        created_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    PatientAcceptedFollowUp {
        previous_booking_id: String,
        follow_up_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        consultation_start_time: i64,
        consultation_duration_in_second: i32,
        consultation_fee: f64,
        symptoms: String,
        consultation_channel: ConsultationChannel,
        created_at: i64,
    },
    #[serde(rename_all = "camelCase")]
    FollowUpCancelled {
        previous_booking_id: String,
        follow_up_id: String,
        patient_identity: PartialUserIdentity,
        doctor_id: i32,
        created_at: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConsultationEvent {
    PreSessionMessage(PreSessionMessage),
    SessionMessage(SessionMessage),
    PostSessionMessage(PostSessionMessage),
}
