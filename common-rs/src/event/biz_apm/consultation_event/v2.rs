use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::event::biz_apm::{model::ConsultationChannel, patient_in_take::PatientInTakeData};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Icd10 {
    pub code: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DurationUnit {
    pub unit: String,
    pub value: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DrugAllergy {
    pub id: i32,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorNoteV2 {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prescription_id: Option<i64>,
    pub present_illness: String,
    pub chief_complaint: String,
    pub diagnosis: String,
    pub recommendations: String,
    #[serde(rename = "icd10")]
    pub icd10: Vec<Icd10>,
    pub illness_duration: DurationUnit,
    pub note_to_staff: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drug_allergies: Option<Vec<DrugAllergy>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum DoctorNote {
    #[serde(rename_all = "camelCase")]
    PlainNote { note: String },
    #[serde(rename_all = "camelCase")]
    DoctorAppV2(DoctorNoteV2),
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "PascalCase")]
pub enum SessionProvider {
    Twilio,
    TokBox,
    Zoom,
}

/// Patient identity with JSON compatibility
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PartialUserIdentity {
    pub account_id: i32,
    pub user_profile_id: i32,
    pub tenant_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oidc_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PartialDoctorInfo {
    pub doctor_id: uuid::Uuid,
    pub first_name: String,
    pub last_name: String,
    pub department_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
pub enum BookingType {
    #[serde(rename = "scheduled")]
    Scheduled,
    #[serde(rename = "instant")]
    Instant,
}

/// Timeslot reserved event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimeslotReservedEvent {
    pub booking_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_identity: PartialUserIdentity,
    pub biz_unit_id: i32,
    pub occupy_from: i64,
    pub occupy_until: i64,
    pub consultation_channel: ConsultationChannel,
    pub reserved_at: i64,
}

/// Reservation cancelled event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReservationCancelledEvent {
    pub booking_id: String,
    pub cancelled_by: String,
    pub cancelled_at: i64,
}

/// Reservation expired event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReservationExpiredEvent {
    pub booking_id: String,
    pub expired_at: i64,
}

/// Consultation booked event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationBookedEvent {
    pub booking_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_identity: PartialUserIdentity,
    pub doctor_info: PartialDoctorInfo,
    pub biz_unit_id: i32,
    pub booking_type: BookingType,
    pub consultation_start_time: i64,
    pub consultation_duration_sec: i32,
    pub consultation_channel: ConsultationChannel,
    pub patient_in_take: PatientInTakeData,
    pub consultation_fee: f64,
    pub platform_fee: f64,
    pub booked_at: i64,
    pub confirmed_at: i64,
    pub payment_transaction_id: i32,
}

/// Consultation cancelled event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationCancelledEvent {
    pub booking_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_identity: PartialUserIdentity,
    pub biz_unit_id: i32,
    pub payment_transaction_id: i32,
    pub cancel_code: String,
    pub cancelled_at: i64,
}

/// Session created event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreatedEvent {
    pub booking_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_identity: PartialUserIdentity,
    pub session_provider: String,
    pub consultation_start_time: i64,
    pub consultation_duration_in_second: i32,
    pub created_at: i64,
}

/// Patient joined event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatientJoinedEvent {
    pub booking_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_identity: PartialUserIdentity,
    pub joined_at: i64,
}

/// Doctor joined event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorJoinedEvent {
    pub booking_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_identity: PartialUserIdentity,
    pub joined_at: i64,
}

/// All participant joined event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AllParticipantJoinedEvent {
    pub booking_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_identity: PartialUserIdentity,
    pub patient_joined_at: i64,
    pub doctor_joined_at: i64,
}

/// Patient disconnected event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatientDisconnectedEvent {
    pub booking_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_identity: PartialUserIdentity,
    pub disconnected_at: i64,
}

/// Doctor disconnected event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorDisconnectedEvent {
    pub booking_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_identity: PartialUserIdentity,
    pub disconnected_at: i64,
}

/// Session terminated event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionTerminatedEvent {
    pub booking_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_identity: PartialUserIdentity,
    pub termination_code: TerminationCode,
    pub terminated_by: SessionParticipant,
    pub terminated_at: i64,
}

/// Consultation summarized event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationSummarizedEvent {
    pub booking_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_identity: PartialUserIdentity,
    pub doctor_note: String,
    pub prescription_info: PrescriptionInfo,
    pub summarized_at: i64,
}

/// Follow up required event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpRequiredEvent {
    pub previous_booking_id: String,
    pub follow_up_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_id: Uuid,
    pub biz_unit_id: i32,
    pub consultation_start_time: i64,
    pub consultation_duration_in_second: i32,
    pub consultation_fee: f64,
    pub consultation_channel: ConsultationChannel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_patient_note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub internal_note: Option<String>,
    pub created_at: i64,
}

/// Follow up request expired event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpRequestExpiredEvent {
    pub previous_booking_id: String,
    pub follow_up_id: String,
    pub doctor_id: Uuid,
    pub patient_identity: PartialUserIdentity,
    pub created_at: i64,
}

/// Patient accepted follow up event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatientAcceptedFollowUpEvent {
    pub previous_booking_id: String,
    pub follow_up_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_id: Uuid,
    pub consultation_start_time: i64,
    pub consultation_duration_in_second: i32,
    pub consultation_fee: f64,
    pub symptoms: String,
    pub consultation_channel: ConsultationChannel,
    pub created_at: i64,
}

/// Follow up cancelled event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpCancelledEvent {
    pub previous_booking_id: String,
    pub follow_up_id: String,
    pub patient_identity: PartialUserIdentity,
    pub doctor_id: Uuid,
    pub created_at: i64,
}

// ===== Supporting types =====

/// Session participant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type", rename_all = "camelCase")]
pub enum SessionParticipant {
    Patient,
    Doctor,
    System,
}

/// Termination code
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
pub enum TerminationCode {
    SuccessfulSession,
    PatientAbsent,
    #[serde(rename_all = "camelCase")]
    DoctorAbsent {
        patient_joined_at: i64,
    },
    BothPartiesAbsent,
    #[serde(rename_all = "camelCase")]
    TechnicalError {
        error_message: String,
    },
    PatientVerificationMissMatch,
}

/// Medicine info
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Medicine {
    pub medicine_id: i32,
    pub price_plan_id: i32,
    pub medicine_amount: i32,
    pub medicine_name_th: String,
    pub medicine_name_en: String,
    pub medicine_instruction_en: String,
    pub medicine_instruction_th: String,
    pub medicine_image_url: String,
    pub medicine_unit_price: f64,
}

/// Prescription info
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PrescriptionInfo {
    pub prescription_refcode: String,
    pub medicine_items: Vec<Medicine>,
    pub expire_at: i64,
}

// ===== Main ConsultationEvent enum =====

/// Consultation event with JSON compatibility
///
/// Uses __type tag for backward compatibility with existing JSON format
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
pub enum BizApmConsultationEvent {
    // == Reservation
    #[serde(rename = "TimeslotReserved")]
    TimeslotReserved(TimeslotReservedEvent),
    #[serde(rename = "ReservationCancelled")]
    ReservationCancelled(ReservationCancelledEvent),
    #[serde(rename = "ReservationExpired")]
    ReservationExpired(ReservationExpiredEvent),
    #[serde(rename = "ConsultationBooked")]
    ConsultationBooked(ConsultationBookedEvent),
    #[serde(rename = "ConsultationCancelled")]
    ConsultationCancelled(ConsultationCancelledEvent),
    // == During consultation
    #[serde(rename = "SessionCreated")]
    SessionCreated(SessionCreatedEvent),
    #[serde(rename = "PatientJoined")]
    PatientJoined(PatientJoinedEvent),
    #[serde(rename = "DoctorJoined")]
    DoctorJoined(DoctorJoinedEvent),
    #[serde(rename = "AllParticipantJoined")]
    AllParticipantJoined(AllParticipantJoinedEvent),
    #[serde(rename = "PatientDisconnected")]
    PatientDisconnected(PatientDisconnectedEvent),
    #[serde(rename = "DoctorDisconnected")]
    DoctorDisconnected(DoctorDisconnectedEvent),
    #[serde(rename = "SessionTerminated")]
    SessionTerminated(SessionTerminatedEvent),
    // == Post
    #[serde(rename = "FollowUpRequired")]
    FollowUpRequired(FollowUpRequiredEvent),
    #[serde(rename = "FollowUpRequestExpired")]
    FollowUpRequestExpired(FollowUpRequestExpiredEvent),
    #[serde(rename = "PatientAcceptedFollowUp")]
    PatientAcceptedFollowUp(PatientAcceptedFollowUpEvent),
    #[serde(rename = "FollowUpCancelled")]
    FollowUpCancelled(FollowUpCancelledEvent),
    #[serde(rename = "ConsultationSummarized")]
    ConsultationSummarized(ConsultationSummarizedEvent),
}

impl BizApmConsultationEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::TimeslotReserved(_) => "TimeslotReserved",
            Self::ReservationCancelled(_) => "ReservationCancelled",
            Self::ReservationExpired(_) => "ReservationExpired",
            Self::ConsultationBooked(_) => "ConsultationBooked",
            Self::ConsultationCancelled(_) => "ConsultationCancelled",
            Self::SessionCreated(_) => "SessionCreated",
            Self::PatientJoined(_) => "PatientJoined",
            Self::DoctorJoined(_) => "DoctorJoined",
            Self::AllParticipantJoined(_) => "AllParticipantJoined",
            Self::PatientDisconnected(_) => "PatientDisconnected",
            Self::DoctorDisconnected(_) => "DoctorDisconnected",
            Self::SessionTerminated(_) => "SessionTerminated",
            Self::FollowUpRequired(_) => "FollowUpRequired",
            Self::FollowUpRequestExpired(_) => "FollowUpRequestExpired",
            Self::PatientAcceptedFollowUp(_) => "PatientAcceptedFollowUp",
            Self::FollowUpCancelled(_) => "FollowUpCancelled",
            Self::ConsultationSummarized(_) => "ConsultationSummarized",
        }
    }

    pub fn aggregate_id(&self) -> &str {
        match self {
            Self::TimeslotReserved(event) => &event.booking_id,
            Self::ReservationCancelled(event) => &event.booking_id,
            Self::ReservationExpired(event) => &event.booking_id,
            Self::ConsultationBooked(event) => &event.booking_id,
            Self::ConsultationCancelled(event) => &event.booking_id,
            Self::SessionCreated(event) => &event.booking_id,
            Self::PatientJoined(event) => &event.booking_id,
            Self::DoctorJoined(event) => &event.booking_id,
            Self::AllParticipantJoined(event) => &event.booking_id,
            Self::PatientDisconnected(event) => &event.booking_id,
            Self::DoctorDisconnected(event) => &event.booking_id,
            Self::SessionTerminated(event) => &event.booking_id,
            Self::ConsultationSummarized(event) => &event.booking_id,
            Self::FollowUpRequired(event) => &event.previous_booking_id,
            Self::FollowUpRequestExpired(event) => &event.previous_booking_id,
            Self::PatientAcceptedFollowUp(event) => &event.previous_booking_id,
            Self::FollowUpCancelled(event) => &event.previous_booking_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn reservation_expired_uses_the_contract_expired_at_field_name() {
        let event = BizApmConsultationEvent::ReservationExpired(ReservationExpiredEvent {
            booking_id: "booking-1".to_string(),
            expired_at: 1_700_000_000,
        });

        let json = serde_json::to_value(event).expect("event should serialize");

        assert_eq!(json["__type"], "ReservationExpired");
        assert_eq!(json["expiredAt"], 1_700_000_000);
        assert!(json.get("expireedAt").is_none());
    }

    #[test]
    fn every_v2_event_round_trips_the_tagged_camel_case_contract() {
        for (event_type, payload) in v2_event_contract_cases() {
            let event: BizApmConsultationEvent = serde_json::from_value(payload.clone())
                .expect("contract payload should deserialize");
            let serialized = serde_json::to_value(event).expect("event should serialize");

            assert_eq!(
                serialized, payload,
                "{event_type} payload drifted from its contract"
            );
            assert_eq!(serialized["__type"], event_type);
        }
    }

    #[test]
    fn doctor_note_v2_uses_camel_case_fields() {
        let note = DoctorNoteV2 {
            prescription_id: Some(1),
            present_illness: "illness".to_string(),
            chief_complaint: "complaint".to_string(),
            diagnosis: "diagnosis".to_string(),
            recommendations: "rest".to_string(),
            icd10: vec![],
            illness_duration: DurationUnit {
                unit: "day".to_string(),
                value: 1,
            },
            note_to_staff: "staff".to_string(),
            drug_allergies: None,
        };

        let json = serde_json::to_value(note).expect("note should serialize");

        assert_eq!(json["prescriptionId"], 1);
        assert_eq!(json["presentIllness"], "illness");
        assert_eq!(json["chiefComplaint"], "complaint");
        assert_eq!(json["illnessDuration"]["unit"], "day");
        assert_eq!(json["noteToStaff"], "staff");
        assert!(json.get("drugAllergies").is_none());
    }

    fn v2_event_contract_cases() -> Vec<(&'static str, Value)> {
        let patient = json!({"accountId": 1, "userProfileId": 2, "tenantId": 3});
        let doctor = json!({"accountId": 4, "userProfileId": 5, "tenantId": 3});
        let prescription = json!({
            "prescriptionRefcode": "RX1",
            "medicineItems": [{
                "medicineId": 1, "pricePlanId": 2, "medicineAmount": 3,
                "medicineNameTh": "th", "medicineNameEn": "en",
                "medicineInstructionEn": "take", "medicineInstructionTh": "take",
                "medicineImageUrl": "https://example.test/medicine", "medicineUnitPrice": 12.5
            }],
            "expireAt": 99
        });

        vec![
            (
                "TimeslotReserved",
                json!({"__type":"TimeslotReserved","bookingId":"B1","patientIdentity":patient,"doctorIdentity":doctor,"bizUnitId":6,"occupyFrom":7,"occupyUntil":8,"consultationChannel":"video","reservedAt":9}),
            ),
            (
                "ReservationCancelled",
                json!({"__type":"ReservationCancelled","bookingId":"B1","cancelledBy":"patient","cancelledAt":9}),
            ),
            (
                "ReservationExpired",
                json!({"__type":"ReservationExpired","bookingId":"B1","expiredAt":9}),
            ),
            (
                "ConsultationBooked",
                json!({"__type":"ConsultationBooked","bookingId":"B1","patientIdentity":patient,"doctorIdentity":doctor,"doctorInfo":{"doctorId":"00000000-0000-0000-0000-000000000002","firstName":"Doctor","lastName":"Who","departmentId":7},"bizUnitId":6,"bookingType":{"__type":"scheduled"},"consultationStartTime":10,"consultationDurationSec":900,"consultationChannel":"video","patientInTake":{"MordeeApp":{"symptom":"rash","duration":1,"durationUnit":"day","attachments":[],"allergies":[]}},"consultationFee":100.0,"platformFee":10.0,"bookedAt":11,"confirmedAt":12,"paymentTransactionId":13}),
            ),
            (
                "ConsultationCancelled",
                json!({"__type":"ConsultationCancelled","bookingId":"B1","patientIdentity":patient,"doctorIdentity":doctor,"bizUnitId":6,"paymentTransactionId":13,"cancelCode":"cancelled","cancelledAt":14}),
            ),
            (
                "SessionCreated",
                json!({"__type":"SessionCreated","bookingId":"B1","patientIdentity":patient,"doctorIdentity":doctor,"sessionProvider":"Twilio","consultationStartTime":10,"consultationDurationInSecond":900,"createdAt":11}),
            ),
            (
                "PatientJoined",
                json!({"__type":"PatientJoined","bookingId":"B1","patientIdentity":patient,"doctorIdentity":doctor,"joinedAt":12}),
            ),
            (
                "DoctorJoined",
                json!({"__type":"DoctorJoined","bookingId":"B1","patientIdentity":patient,"doctorIdentity":doctor,"joinedAt":12}),
            ),
            (
                "AllParticipantJoined",
                json!({"__type":"AllParticipantJoined","bookingId":"B1","patientIdentity":patient,"doctorIdentity":doctor,"patientJoinedAt":12,"doctorJoinedAt":13}),
            ),
            (
                "PatientDisconnected",
                json!({"__type":"PatientDisconnected","bookingId":"B1","patientIdentity":patient,"doctorIdentity":doctor,"disconnectedAt":14}),
            ),
            (
                "DoctorDisconnected",
                json!({"__type":"DoctorDisconnected","bookingId":"B1","patientIdentity":patient,"doctorIdentity":doctor,"disconnectedAt":14}),
            ),
            (
                "SessionTerminated",
                json!({"__type":"SessionTerminated","bookingId":"B1","patientIdentity":patient,"doctorIdentity":doctor,"terminationCode":{"__type":"DoctorAbsent","patientJoinedAt":12},"terminatedBy":{"__type":"system"},"terminatedAt":15}),
            ),
            (
                "FollowUpRequired",
                json!({"__type":"FollowUpRequired","previousBookingId":"B1","followUpId":"F1","patientIdentity":patient,"doctorId":"00000000-0000-0000-0000-000000000001","bizUnitId":6,"consultationStartTime":10,"consultationDurationInSecond":900,"consultationFee":100.0,"consultationChannel":"video","createdAt":16}),
            ),
            (
                "FollowUpRequestExpired",
                json!({"__type":"FollowUpRequestExpired","previousBookingId":"B1","followUpId":"F1","doctorId":"00000000-0000-0000-0000-000000000001","patientIdentity":patient,"createdAt":16}),
            ),
            (
                "PatientAcceptedFollowUp",
                json!({"__type":"PatientAcceptedFollowUp","previousBookingId":"B1","followUpId":"F1","patientIdentity":patient,"doctorId":"00000000-0000-0000-0000-000000000001","consultationStartTime":10,"consultationDurationInSecond":900,"consultationFee":100.0,"symptoms":"rash","consultationChannel":"video","createdAt":16}),
            ),
            (
                "FollowUpCancelled",
                json!({"__type":"FollowUpCancelled","previousBookingId":"B1","followUpId":"F1","patientIdentity":patient,"doctorId":"00000000-0000-0000-0000-000000000001","createdAt":16}),
            ),
            (
                "ConsultationSummarized",
                json!({"__type":"ConsultationSummarized","bookingId":"B1","patientIdentity":patient,"doctorIdentity":doctor,"doctorNote":"summary","prescriptionInfo":prescription,"summarizedAt":17}),
            ),
        ]
    }
}
