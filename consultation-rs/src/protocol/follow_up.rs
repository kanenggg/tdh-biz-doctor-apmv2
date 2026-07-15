use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::ConsultationChannel;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum VisitType {
    #[serde(rename = "FollowUp")]
    FollowUp,
    #[serde(rename = "LabResult")]
    LabResult,
    #[serde(rename = "PrescriptionRefill")]
    PrescriptionRefill,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FollowUpAppointment {
    pub parent_booking_id: String,
    pub appointment_start: i64,
    pub appointment_end: i64,
    pub visit_types: Vec<VisitType>,
    pub additional_note_to_patient: String,
    pub note_to_staff: String,
    pub consultation_channel: ConsultationChannel,
    pub consultation_fee: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
pub enum FollowUp {
    AsNeeded,
    Appointment(FollowUpAppointment),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_follow_up_appointment_consultation_fee_from_json() {
        let json = r#"{
            "__type": "Appointment",
            "parentBookingId": "booking-123",
            "appointmentStart": 1700000000,
            "appointmentEnd": 1700003600,
            "visitTypes": ["FollowUp"],
            "additionalNoteToPatient": "",
            "noteToStaff": "",
            "consultationChannel": "video",
            "consultationFee": 500.50
        }"#;

        let follow_up: FollowUp = serde_json::from_str(json).unwrap();
        match follow_up {
            FollowUp::Appointment(apt) => {
                assert!((apt.consultation_fee - 500.50).abs() < f64::EPSILON);
            }
            _ => panic!("expected Appointment variant"),
        }
    }

    #[test]
    fn test_follow_up_appointment_consultation_fee_integer_from_json() {
        let json = r#"{
            "__type": "Appointment",
            "parentBookingId": "booking-456",
            "appointmentStart": 1700000000,
            "appointmentEnd": 1700003600,
            "visitTypes": [],
            "additionalNoteToPatient": "",
            "noteToStaff": "",
            "consultationChannel": "chat",
            "consultationFee": 1000
        }"#;

        let follow_up: FollowUp = serde_json::from_str(json).unwrap();
        match follow_up {
            FollowUp::Appointment(apt) => {
                assert!((apt.consultation_fee - 1000.0).abs() < f64::EPSILON);
            }
            _ => panic!("expected Appointment variant"),
        }
    }

    #[test]
    fn test_follow_up_appointment_consultation_fee_zero() {
        let json = r#"{
            "__type": "Appointment",
            "parentBookingId": "booking-789",
            "appointmentStart": 1700000000,
            "appointmentEnd": 1700003600,
            "visitTypes": ["LabResult"],
            "additionalNoteToPatient": "note",
            "noteToStaff": "staff note",
            "consultationChannel": "voice",
            "consultationFee": 0
        }"#;

        let follow_up: FollowUp = serde_json::from_str(json).unwrap();
        match follow_up {
            FollowUp::Appointment(apt) => {
                assert_eq!(apt.consultation_fee, 0.0);
            }
            _ => panic!("expected Appointment variant"),
        }
    }

    #[test]
    fn test_follow_up_as_needed_from_json() {
        let json = r#"{"__type": "AsNeeded"}"#;
        let follow_up: FollowUp = serde_json::from_str(json).unwrap();
        assert!(matches!(follow_up, FollowUp::AsNeeded));
    }

    #[test]
    fn test_visit_type_prescription_refill_uses_correct_wire_value() {
        let json = serde_json::to_string(&VisitType::PrescriptionRefill).unwrap();
        assert_eq!(json, r#""PrescriptionRefill""#);

        let visit_type: VisitType = serde_json::from_str(r#""PrescriptionRefill""#).unwrap();
        assert!(matches!(visit_type, VisitType::PrescriptionRefill));
    }

    #[test]
    fn test_follow_up_appointment_roundtrip() {
        let apt = FollowUp::Appointment(FollowUpAppointment {
            parent_booking_id: "b-1".to_string(),
            appointment_start: 1700000000,
            appointment_end: 1700003600,
            visit_types: vec![VisitType::FollowUp],
            additional_note_to_patient: String::new(),
            note_to_staff: String::new(),
            consultation_channel: ConsultationChannel::Video,
            consultation_fee: 299.99,
        });

        let json = serde_json::to_string(&apt).unwrap();
        let deserialized: FollowUp = serde_json::from_str(&json).unwrap();

        match deserialized {
            FollowUp::Appointment(result) => {
                assert!((result.consultation_fee - 299.99).abs() < f64::EPSILON);
            }
            _ => panic!("expected Appointment variant"),
        }
    }
}
