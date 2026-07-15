use crate::tdh_protocol::{
    appointment::v2::payment_transaction::PaymentChannels,
    appointment::AppointmentStatus,
    common::PartialUserIdentity,
    consultation::{
        consultation_pre_screen::ConsultationPreScreen, BookingType, ConsultationChannel,
    },
};

fn default_consult_duration() -> Option<i32> {
    Some(20)
}
fn default_booking_type() -> BookingType {
    BookingType::Instant
}

fn default_consultation_channel() -> ConsultationChannel {
    ConsultationChannel::Video
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "sqlx", derive(sqlx::FromRow))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateConfirmedInstantAppointmentRequest {
    pub biz_unit_id: i32,
    pub biz_center_id: i32,
    pub tenant_id: i32,
    pub patient_id: PartialUserIdentity,
    pub doctor_id: PartialUserIdentity,
    pub prescreen: ConsultationPreScreen,
    #[serde(
        skip_serializing_if = "Option::is_none",
        default = "default_consult_duration"
    )]
    pub consult_duration: Option<i32>,
    #[serde(default = "default_booking_type")]
    pub booking_type: BookingType,
    #[serde(default = "default_consultation_channel")]
    pub consultation_channel: ConsultationChannel,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_appointment_id: Option<String>,
    pub payment_channels: PaymentChannels,
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAppointmentSelectedTimeslot {
    pub timeslot_id: String,
    pub start_epoch: i64,
    pub end_epoch: i64,
    pub consultation_channel: ConsultationChannel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateAppointmentValidationError {
    MissingAppointmentNo,
    MissingPrescreen,
    MissingPrescreenSymptom,
    InvalidPrescreenDuration,
    MissingPrescreenDurationUnit,
    InvalidAppointmentWindow,
    MissingSelectedTimeslot,
    MissingSelectedTimeslotId,
    SelectedTimeslotWindowMismatch,
    SelectedTimeslotChannelMismatch,
}

impl CreateAppointmentValidationError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::MissingAppointmentNo => {
                "appointmentNo is required for Star Gate appointment creation"
            }
            Self::MissingPrescreen => "prescreen is required for Star Gate appointment creation",
            Self::MissingPrescreenSymptom => "prescreen.symptom is required",
            Self::InvalidPrescreenDuration => "prescreen.duration must be greater than zero",
            Self::MissingPrescreenDurationUnit => "prescreen.durationUnit is required",
            Self::InvalidAppointmentWindow => {
                "appointmentEnd must be greater than appointmentStart"
            }
            Self::MissingSelectedTimeslot => {
                "selectedTimeslot is required when bookingType is Schedule"
            }
            Self::MissingSelectedTimeslotId => "selectedTimeslot.timeslotId is required",
            Self::SelectedTimeslotWindowMismatch => {
                "selectedTimeslot start/end must match appointmentStart/appointmentEnd"
            }
            Self::SelectedTimeslotChannelMismatch => {
                "selectedTimeslot.consultationChannel must match consultationChannel"
            }
        }
    }
}

impl std::fmt::Display for CreateAppointmentValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for CreateAppointmentValidationError {}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAppointmentRequest {
    pub biz_unit_id: i32,
    pub biz_center_id: i32,
    pub tenant_id: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appointment_no: Option<String>,
    pub patient_id: PartialUserIdentity,
    pub doctor_id: PartialUserIdentity,
    #[serde(default = "default_booking_type")]
    pub booking_type: BookingType,
    #[serde(default = "default_consultation_channel")]
    pub consultation_channel: ConsultationChannel,
    pub appointment_start: i64,
    pub appointment_end: i64,
    pub appointment_status: AppointmentStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_tx_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_tx_ref_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payment_channels: Option<PaymentChannels>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_appointment_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prescreen: Option<ConsultationPreScreen>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_timeslot: Option<CreateAppointmentSelectedTimeslot>,
}

impl CreateAppointmentRequest {
    pub fn validate_star_gate_creation(&self) -> Result<(), CreateAppointmentValidationError> {
        if self
            .appointment_no
            .as_deref()
            .is_none_or(|appointment_no| appointment_no.trim().is_empty())
        {
            return Err(CreateAppointmentValidationError::MissingAppointmentNo);
        }

        if self.appointment_end <= self.appointment_start {
            return Err(CreateAppointmentValidationError::InvalidAppointmentWindow);
        }

        let prescreen = self
            .prescreen
            .as_ref()
            .ok_or(CreateAppointmentValidationError::MissingPrescreen)?;
        if prescreen.symptom.trim().is_empty() {
            return Err(CreateAppointmentValidationError::MissingPrescreenSymptom);
        }
        if prescreen.duration <= 0 {
            return Err(CreateAppointmentValidationError::InvalidPrescreenDuration);
        }
        if prescreen.duration_unit.trim().is_empty() {
            return Err(CreateAppointmentValidationError::MissingPrescreenDurationUnit);
        }

        if matches!(self.booking_type, BookingType::Schedule) {
            let timeslot = self
                .selected_timeslot
                .as_ref()
                .ok_or(CreateAppointmentValidationError::MissingSelectedTimeslot)?;
            if timeslot.timeslot_id.trim().is_empty() {
                return Err(CreateAppointmentValidationError::MissingSelectedTimeslotId);
            }
            if timeslot.start_epoch != self.appointment_start
                || timeslot.end_epoch != self.appointment_end
            {
                return Err(CreateAppointmentValidationError::SelectedTimeslotWindowMismatch);
            }
            if timeslot.consultation_channel != self.consultation_channel {
                return Err(CreateAppointmentValidationError::SelectedTimeslotChannelMismatch);
            }
        }

        Ok(())
    }
}

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAppointmentResult {
    pub booking_id: String,
    pub appointment_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appointment_no: Option<String>,
    pub ref_code: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tdh_protocol::appointment::AppointmentStatus;

    #[test]
    fn create_appointment_request_deserializes_nested_serde_shape() {
        let request: CreateAppointmentRequest = serde_json::from_value(serde_json::json!({
            "bizUnitId": 1,
            "bizCenterId": 100,
            "tenantId": 1,
            "patientId": {
                "accountId": 5001,
                "userProfileId": 6001,
                "tenantId": 1
            },
            "doctorId": {
                "accountId": 7001,
                "userProfileId": 8001,
                "tenantId": 1
            },
            "appointmentStart": 1_771_000_000_i64,
            "appointmentEnd": 1_771_001_200_i64,
            "appointmentStatus": "BOOKED",
            "paymentTxId": 42
        }))
        .unwrap();

        assert_eq!(request.patient_id.account_id, 5001);
        assert_eq!(request.doctor_id.user_profile_id, 8001);
        assert!(matches!(request.booking_type, BookingType::Instant));
        assert_eq!(request.consultation_channel, ConsultationChannel::Video);
        assert_eq!(request.appointment_status, AppointmentStatus::Booked);
        assert!(request.appointment_no.is_none());
        assert!(request.payment_channels.is_none());
        assert!(request.prescreen.is_none());
    }

    #[test]
    fn create_appointment_request_supports_appointment_no() {
        let request: CreateAppointmentRequest = serde_json::from_value(serde_json::json!({
            "bizUnitId": 1,
            "bizCenterId": 100,
            "tenantId": 1,
            "patientId": {
                "accountId": 5001,
                "userProfileId": 6001,
                "tenantId": 1
            },
            "doctorId": {
                "accountId": 7001,
                "userProfileId": 8001,
                "tenantId": 1
            },
            "appointmentStart": 1_771_000_000_i64,
            "appointmentEnd": 1_771_001_200_i64,
            "appointmentStatus": "BOOKED",
            "paymentTxId": 42,
            "appointmentNo": "20260522100001"
        }))
        .unwrap();

        assert_eq!(request.appointment_no.as_deref(), Some("20260522100001"));

        let json = serde_json::to_value(request).unwrap();
        assert_eq!(json["appointmentNo"], "20260522100001");
    }

    fn minimal_request(booking_type: BookingType) -> CreateAppointmentRequest {
        CreateAppointmentRequest {
            biz_unit_id: 1,
            biz_center_id: 100,
            tenant_id: 1,
            appointment_no: Some("SG-20260710-000001".to_string()),
            patient_id: PartialUserIdentity {
                account_id: 5001,
                user_profile_id: 6001,
                tenant_id: 1,
                oidc_user_id: None,
            },
            doctor_id: PartialUserIdentity {
                account_id: 7001,
                user_profile_id: 8001,
                tenant_id: 1,
                oidc_user_id: None,
            },
            booking_type,
            consultation_channel: ConsultationChannel::Video,
            appointment_start: 1_771_000_000,
            appointment_end: 1_771_001_200,
            appointment_status: AppointmentStatus::Booked,
            payment_tx_id: None,
            payment_tx_ref_id: Some("PAY-REF-1".to_string()),
            payment_channels: None,
            parent_appointment_id: None,
            prescreen: Some(ConsultationPreScreen {
                symptom: "headache".to_string(),
                duration: 3,
                duration_unit: "day".to_string(),
                attachments: vec![],
                allergies: vec![],
            }),
            selected_timeslot: None,
        }
    }

    #[test]
    fn create_appointment_request_serializes_camel_case_fields() {
        let mut request = minimal_request(BookingType::Instant);
        request.appointment_no = None;
        request.payment_tx_ref_id = None;
        request.prescreen = None;

        let json = serde_json::to_value(request).unwrap();

        assert_eq!(json["bizUnitId"], 1);
        assert_eq!(json["patientId"]["accountId"], 5001);
        assert_eq!(json["doctorId"]["userProfileId"], 8001);
        assert_eq!(json["appointmentStatus"], "BOOKED");
        assert!(json.get("appointmentNo").is_none());
        assert!(json.get("paymentTxId").is_none());
        assert!(json.get("paymentChannels").is_none());
        assert!(json.get("prescreen").is_none());
        assert!(json.get("selectedTimeslot").is_none());
    }

    #[test]
    fn star_gate_instant_creation_contract_validates_prescreen_and_ref_code() {
        let request = minimal_request(BookingType::Instant);

        request.validate_star_gate_creation().unwrap();

        let json = serde_json::to_value(request).unwrap();
        assert_eq!(json["bookingType"]["__type"], "Instant");
        assert_eq!(json["appointmentNo"], "SG-20260710-000001");
        assert_eq!(json["prescreen"]["symptom"], "headache");
        assert_eq!(json["prescreen"]["duration"], 3);
        assert_eq!(json["prescreen"]["durationUnit"], "day");
    }

    #[test]
    fn star_gate_scheduled_creation_requires_selected_timeslot_matching_request_window() {
        let mut request = minimal_request(BookingType::Schedule);
        request.selected_timeslot = Some(CreateAppointmentSelectedTimeslot {
            timeslot_id: "doctor-uuid:1771000000:1771001200:video".to_string(),
            start_epoch: 1_771_000_000,
            end_epoch: 1_771_001_200,
            consultation_channel: ConsultationChannel::Video,
        });

        request.validate_star_gate_creation().unwrap();

        let json = serde_json::to_value(request).unwrap();
        assert_eq!(json["bookingType"]["__type"], "Schedule");
        assert_eq!(
            json["selectedTimeslot"]["timeslotId"],
            "doctor-uuid:1771000000:1771001200:video"
        );
        assert_eq!(json["selectedTimeslot"]["consultationChannel"], "video");
    }

    #[test]
    fn star_gate_creation_rejects_missing_appointment_no() {
        let mut request = minimal_request(BookingType::Instant);
        request.appointment_no = None;

        let error = request.validate_star_gate_creation().unwrap_err();

        assert_eq!(
            error,
            CreateAppointmentValidationError::MissingAppointmentNo
        );
    }

    #[test]
    fn star_gate_creation_rejects_blank_prescreen_symptom() {
        let mut request = minimal_request(BookingType::Instant);
        request.prescreen.as_mut().unwrap().symptom = "   ".to_string();

        let error = request.validate_star_gate_creation().unwrap_err();

        assert_eq!(
            error,
            CreateAppointmentValidationError::MissingPrescreenSymptom
        );
    }

    #[test]
    fn star_gate_creation_rejects_scheduled_request_without_selected_timeslot() {
        let request = minimal_request(BookingType::Schedule);

        let error = request.validate_star_gate_creation().unwrap_err();

        assert_eq!(
            error,
            CreateAppointmentValidationError::MissingSelectedTimeslot
        );
    }

    #[test]
    fn create_appointment_result_returns_booking_appointment_and_ref_code_identifiers() {
        let result = CreateAppointmentResult {
            booking_id: "SG-20260710-000001".to_string(),
            appointment_id: "SG-20260710-000001".to_string(),
            appointment_no: Some("SG-20260710-000001".to_string()),
            ref_code: "SG-20260710-000001".to_string(),
        };

        let json = serde_json::to_value(result).unwrap();

        assert_eq!(json["bookingId"], "SG-20260710-000001");
        assert_eq!(json["appointmentId"], "SG-20260710-000001");
        assert_eq!(json["appointmentNo"], "SG-20260710-000001");
        assert_eq!(json["refCode"], "SG-20260710-000001");
    }
}
