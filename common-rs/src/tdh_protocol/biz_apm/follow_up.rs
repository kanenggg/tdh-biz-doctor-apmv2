use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::tdh_protocol::biz_apm::ConsultationChannel;

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
    fn test_visit_type_prescription_refill_uses_correct_wire_value() {
        let json = serde_json::to_string(&VisitType::PrescriptionRefill).unwrap();
        assert_eq!(json, r#""PrescriptionRefill""#);

        let visit_type: VisitType = serde_json::from_str(r#""PrescriptionRefill""#).unwrap();
        assert!(matches!(visit_type, VisitType::PrescriptionRefill));
    }
}
