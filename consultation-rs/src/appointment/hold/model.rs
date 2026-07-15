use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::common::tdh_protocol::{
    appointment::reserve::{PatientPrescreen, Timeslot},
    appointment::{ReserveRequest, ReserveResponse},
    common::patient_identity::PartialUserIdentity,
    consultation::{BookingType, ConsultationChannel},
};

/// Canonical input to Appointment Hold creation.  `ReserveRequest` remains a
/// protocol adapter type; handlers convert it at the HTTP boundary.
#[derive(Debug, Clone)]
pub struct CreateAppointmentHold {
    pub doctor_id: i32,
    pub biz_unit_id: i32,
    pub biz_center_id: i32,
    pub patient_intake: PatientPrescreen,
    pub consultation_channel: ConsultationChannel,
    pub timeslot: Timeslot,
    pub booking_type: BookingType,
    pub trace_id: Option<String>,
}

impl From<ReserveRequest> for CreateAppointmentHold {
    fn from(value: ReserveRequest) -> Self {
        Self {
            doctor_id: value.doctor_id,
            biz_unit_id: value.biz_unit_id,
            biz_center_id: value.biz_center_id,
            patient_intake: value.patient_intake,
            consultation_channel: value.consultation_channel,
            timeslot: value.timeslot,
            booking_type: value.booking_type,
            trace_id: value.trace_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppointmentHoldCreated {
    pub booking_id: String,
    pub payment_quote: PaymentQuote,
}

/// Server-owned immutable payment facts.  This is additive to the legacy
/// booking response so payment producers never derive a price from a mutable
/// doctor profile or a client display value.
#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PaymentQuote {
    pub amount: String,
    pub currency: String,
    pub effective_service_config_version: i64,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PublicBookingRequest {
    #[serde(flatten)]
    pub reserve: ReserveRequest,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InternalBookingRequest {
    pub patient_identity: PartialUserIdentity,
    #[serde(flatten)]
    pub reserve: ReserveRequest,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BookingResponse {
    pub booking_id: Option<String>,
    pub reserve_token: Option<String>,
    pub status: BookingResponseStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_quote: Option<PaymentQuote>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "PascalCase")]
pub enum BookingResponseStatus {
    Reserved,
    DoctorNotAvailable,
    SlotAlreadyBooked,
}

impl From<ReserveResponse> for BookingResponse {
    fn from(value: ReserveResponse) -> Self {
        match value {
            ReserveResponse::Success(success) => Self {
                booking_id: Some(success.reserve_token.clone()),
                reserve_token: Some(success.reserve_token),
                status: BookingResponseStatus::Reserved,
                payment_quote: None,
            },
            ReserveResponse::DoctorNotAvailable => Self {
                booking_id: None,
                reserve_token: None,
                status: BookingResponseStatus::DoctorNotAvailable,
                payment_quote: None,
            },
            ReserveResponse::SlotAlreadyBooked => Self {
                booking_id: None,
                reserve_token: None,
                status: BookingResponseStatus::SlotAlreadyBooked,
                payment_quote: None,
            },
        }
    }
}

impl From<AppointmentHoldCreated> for BookingResponse {
    fn from(value: AppointmentHoldCreated) -> Self {
        Self {
            booking_id: Some(value.booking_id.clone()),
            reserve_token: Some(value.booking_id),
            status: BookingResponseStatus::Reserved,
            payment_quote: Some(value.payment_quote),
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BookingStateResponse {
    pub booking_id: String,
    pub state: BookingLifecycle,
    pub reservation_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appointment_status: Option<String>,
    pub reserved_until: i64,
    pub appointment_start: i64,
    pub appointment_end: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BookingCancelResponse {
    pub booking_id: String,
    pub state: BookingLifecycle,
    pub cancelled_lifecycle: BookingLifecycle,
    pub cancelled_at: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum BookingLifecycle {
    Reserved,
    ReserveExpired,
    Booked,
    ConsultationDone,
    Cancelled,
    Reservation,
    Appointment,
    Unknown,
}
