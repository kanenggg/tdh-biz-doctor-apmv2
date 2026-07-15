//! Consultation event types with serde JSON support.

use crate::tdh_protocol::biz_apm::common::{BookingType, ConsultationChannel, PatientIdentity};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ===== Event types =====

/// Timeslot reserved event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimeslotReservedEvent {
    pub booking_id: String,
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
    pub biz_unit_id: i32,
    pub reserved_from: i64,
    pub reservation_duration_sec: i32,
    pub consultation_channel: ConsultationChannel,
    pub reserved_at: i64,
}

/// Reservation cancelled event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReservationCancelledEvent {
    pub booking_id: String,
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
    pub biz_unit_id: i32,
    pub cancelled_at: i64,
}

/// Reservation expired event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReservationExpiredEvent {
    pub booking_id: String,
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
    pub biz_unit_id: i32,
    pub cancelled_at: i64,
}

/// Consultation booked event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationBookedEvent {
    pub booking_id: String,
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
    pub biz_unit_id: i32,
    pub payment_module_id: i32,
    pub booking_type: BookingType,
    pub consultation_start_time: i64,
    pub consultation_duration_in_second: i32,
    pub consultation_channel: ConsultationChannel,
    pub booked_at: i64,
    pub symptoms: String,
    pub consultation_fee: f64,
}

/// Consultation cancelled event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationCancelledEvent {
    pub booking_id: String,
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
    pub biz_unit_id: i32,
    pub payment_module_id: i32,
    pub cancel_code: String,
    pub cancelled_at: i64,
}

/// Session created event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionCreatedEvent {
    pub booking_id: String,
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
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
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
    pub joined_at: i64,
}

/// Doctor joined event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorJoinedEvent {
    pub booking_id: String,
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
    pub joined_at: i64,
}

/// All participant joined event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AllParticipantJoinedEvent {
    pub booking_id: String,
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
    pub patient_joined_at: i64,
    pub doctor_joined_at: i64,
}

/// Patient disconnected event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatientDisconnectedEvent {
    pub booking_id: String,
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
    pub disconnected_at: i64,
}

/// Doctor disconnected event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DoctorDisconnectedEvent {
    pub booking_id: String,
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
    pub disconnected_at: i64,
}

/// Session terminated event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionTerminatedEvent {
    pub booking_id: String,
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
    pub termination_code: TerminationCode,
    pub terminated_by: SessionParticipant,
    pub terminated_at: i64,
}

/// Consultation summarized event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationSummarizedEvent {
    pub booking_id: String,
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
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
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
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
    pub doctor_id: i32,
    pub patient_identity: PatientIdentity,
    pub created_at: i64,
}

/// Patient accepted follow up event
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatientAcceptedFollowUpEvent {
    pub previous_booking_id: String,
    pub follow_up_id: String,
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
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
    pub patient_identity: PatientIdentity,
    pub doctor_id: i32,
    pub created_at: i64,
}

// ===== Supporting types =====

/// Session participant
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum SessionParticipant {
    Patient,
    Doctor,
    System,
}

/// Termination code
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
pub enum TerminationCode {
    SuccessfulSession {
        patient_joined_at: i64,
        doctor_joined_at: i64,
    },
    PatientAbsent {
        doctor_joined_at: i64,
    },
    DoctorAbsent {
        patient_joined_at: i64,
    },
    BothPartiesAbsent,
    TechnicalError {
        error_message: String,
    },
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
pub enum ConsultationEvent {
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
    #[serde(rename = "ConsultationSummarized")]
    ConsultationSummarized(ConsultationSummarizedEvent),
    #[serde(rename = "FollowUpRequired")]
    FollowUpRequired(FollowUpRequiredEvent),
    #[serde(rename = "FollowUpRequestExpired")]
    FollowUpRequestExpired(FollowUpRequestExpiredEvent),
    #[serde(rename = "PatientAcceptedFollowUp")]
    PatientAcceptedFollowUp(PatientAcceptedFollowUpEvent),
    #[serde(rename = "FollowUpCancelled")]
    FollowUpCancelled(FollowUpCancelledEvent),
}

impl ConsultationEvent {
    /// Get booking_id from any event variant
    pub fn booking_id(&self) -> String {
        match self {
            ConsultationEvent::TimeslotReserved(e) => e.booking_id.clone(),
            ConsultationEvent::ReservationCancelled(e) => e.booking_id.clone(),
            ConsultationEvent::ReservationExpired(e) => e.booking_id.clone(),
            ConsultationEvent::ConsultationBooked(e) => e.booking_id.clone(),
            ConsultationEvent::ConsultationCancelled(e) => e.booking_id.clone(),
            ConsultationEvent::SessionCreated(e) => e.booking_id.clone(),
            ConsultationEvent::PatientJoined(e) => e.booking_id.clone(),
            ConsultationEvent::DoctorJoined(e) => e.booking_id.clone(),
            ConsultationEvent::AllParticipantJoined(e) => e.booking_id.clone(),
            ConsultationEvent::PatientDisconnected(e) => e.booking_id.clone(),
            ConsultationEvent::DoctorDisconnected(e) => e.booking_id.clone(),
            ConsultationEvent::SessionTerminated(e) => e.booking_id.clone(),
            ConsultationEvent::ConsultationSummarized(e) => e.booking_id.clone(),
            ConsultationEvent::FollowUpRequired(e) => e.previous_booking_id.clone(),
            ConsultationEvent::FollowUpRequestExpired(e) => e.previous_booking_id.clone(),
            ConsultationEvent::PatientAcceptedFollowUp(e) => e.previous_booking_id.clone(),
            ConsultationEvent::FollowUpCancelled(e) => e.previous_booking_id.clone(),
        }
    }

    /// Get the doctor ID for this event
    pub fn doctor_id(&self) -> i32 {
        match self {
            ConsultationEvent::TimeslotReserved(e) => e.doctor_id,
            ConsultationEvent::ReservationCancelled(e) => e.doctor_id,
            ConsultationEvent::ReservationExpired(e) => e.doctor_id,
            ConsultationEvent::ConsultationBooked(e) => e.doctor_id,
            ConsultationEvent::ConsultationCancelled(e) => e.doctor_id,
            ConsultationEvent::SessionCreated(e) => e.doctor_id,
            ConsultationEvent::PatientJoined(e) => e.doctor_id,
            ConsultationEvent::DoctorJoined(e) => e.doctor_id,
            ConsultationEvent::AllParticipantJoined(e) => e.doctor_id,
            ConsultationEvent::PatientDisconnected(e) => e.doctor_id,
            ConsultationEvent::DoctorDisconnected(e) => e.doctor_id,
            ConsultationEvent::SessionTerminated(e) => e.doctor_id,
            ConsultationEvent::ConsultationSummarized(e) => e.doctor_id,
            ConsultationEvent::FollowUpRequired(e) => e.doctor_id,
            ConsultationEvent::FollowUpRequestExpired(e) => e.doctor_id,
            ConsultationEvent::PatientAcceptedFollowUp(e) => e.doctor_id,
            ConsultationEvent::FollowUpCancelled(e) => e.doctor_id,
        }
    }

    /// Get the patient identity for this event
    pub fn patient_identity(&self) -> PatientIdentity {
        match self {
            ConsultationEvent::TimeslotReserved(e) => e.patient_identity.clone(),
            ConsultationEvent::ReservationCancelled(e) => e.patient_identity.clone(),
            ConsultationEvent::ReservationExpired(e) => e.patient_identity.clone(),
            ConsultationEvent::ConsultationBooked(e) => e.patient_identity.clone(),
            ConsultationEvent::ConsultationCancelled(e) => e.patient_identity.clone(),
            ConsultationEvent::SessionCreated(e) => e.patient_identity.clone(),
            ConsultationEvent::PatientJoined(e) => e.patient_identity.clone(),
            ConsultationEvent::DoctorJoined(e) => e.patient_identity.clone(),
            ConsultationEvent::AllParticipantJoined(e) => e.patient_identity.clone(),
            ConsultationEvent::PatientDisconnected(e) => e.patient_identity.clone(),
            ConsultationEvent::DoctorDisconnected(e) => e.patient_identity.clone(),
            ConsultationEvent::SessionTerminated(e) => e.patient_identity.clone(),
            ConsultationEvent::ConsultationSummarized(e) => e.patient_identity.clone(),
            ConsultationEvent::FollowUpRequired(e) => e.patient_identity.clone(),
            ConsultationEvent::FollowUpRequestExpired(e) => e.patient_identity.clone(),
            ConsultationEvent::PatientAcceptedFollowUp(e) => e.patient_identity.clone(),
            ConsultationEvent::FollowUpCancelled(e) => e.patient_identity.clone(),
        }
    }

    /// Get the event type name for this event
    pub fn event_type_name(&self) -> &'static str {
        match self {
            ConsultationEvent::TimeslotReserved(_) => "TimeslotReserved",
            ConsultationEvent::ReservationCancelled(_) => "ReservationCancelled",
            ConsultationEvent::ReservationExpired(_) => "ReservationExpired",
            ConsultationEvent::ConsultationBooked(_) => "ConsultationBooked",
            ConsultationEvent::ConsultationCancelled(_) => "ConsultationCancelled",
            ConsultationEvent::SessionCreated(_) => "SessionCreated",
            ConsultationEvent::PatientJoined(_) => "PatientJoined",
            ConsultationEvent::DoctorJoined(_) => "DoctorJoined",
            ConsultationEvent::AllParticipantJoined(_) => "AllParticipantJoined",
            ConsultationEvent::PatientDisconnected(_) => "PatientDisconnected",
            ConsultationEvent::DoctorDisconnected(_) => "DoctorDisconnected",
            ConsultationEvent::SessionTerminated(_) => "SessionTerminated",
            ConsultationEvent::ConsultationSummarized(_) => "ConsultationSummarized",
            ConsultationEvent::FollowUpRequired(_) => "FollowUpRequired",
            ConsultationEvent::FollowUpRequestExpired(_) => "FollowUpRequestExpired",
            ConsultationEvent::PatientAcceptedFollowUp(_) => "PatientAcceptedFollowUp",
            ConsultationEvent::FollowUpCancelled(_) => "FollowUpCancelled",
        }
    }
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_consultation_event_serialization() {
        let event = ConsultationEvent::TimeslotReserved(TimeslotReservedEvent {
            booking_id: "test".to_string(),
            patient_identity: PatientIdentity {
                account_id: 123,
                user_profile_id: 456,
                tenant_id: 1,
                oidc_user_id: None,
            },
            doctor_id: 789,
            biz_unit_id: 1,
            reserved_from: 0,
            reservation_duration_sec: 0,
            consultation_channel: ConsultationChannel::Video,
            reserved_at: 0,
        });

        let json_value = serde_json::to_value(&event).unwrap();
        assert_eq!(json_value["__type"], "TimeslotReserved");
    }
}
