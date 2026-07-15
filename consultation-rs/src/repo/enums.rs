use crate::common::tdh_protocol::{
    common::meeting_provider::MeetingProvider, consultation::ConsultationChannel,
};
use serde::{Deserialize, Serialize};
use sqlx::Type;
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "lowercase")]
#[schema(rename_all = "lowercase")]
#[sqlx(type_name = "v2.consultation_type_enum")]
pub enum ConsultationChannelEnum {
    #[sqlx(rename = "video")]
    Video,
    #[sqlx(rename = "voice")]
    Voice,
    #[sqlx(rename = "chat")]
    Chat,
}

impl From<ConsultationChannelEnum> for ConsultationChannel {
    fn from(db: ConsultationChannelEnum) -> Self {
        match db {
            ConsultationChannelEnum::Video => ConsultationChannel::Video,
            ConsultationChannelEnum::Voice => ConsultationChannel::Voice,
            ConsultationChannelEnum::Chat => ConsultationChannel::Chat,
        }
    }
}

impl From<ConsultationChannel> for ConsultationChannelEnum {
    fn from(proto: ConsultationChannel) -> Self {
        match proto {
            ConsultationChannel::Video => ConsultationChannelEnum::Video,
            ConsultationChannel::Voice => ConsultationChannelEnum::Voice,
            ConsultationChannel::Chat => ConsultationChannelEnum::Chat,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppointmentStatusEnum, ConsultationChannelEnum};

    #[test]
    fn consultation_channel_enum_serializes_as_lowercase_wire_value() {
        let value = serde_json::to_string(&ConsultationChannelEnum::Video).unwrap();

        assert_eq!(value, "\"video\"");
    }

    #[test]
    fn consultation_done_status_serializes_as_db_wire_value() {
        let value = serde_json::to_string(&AppointmentStatusEnum::ConsultationDone).unwrap();

        assert_eq!(value, "\"CONSULTATION_DONE\"");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Type)]
#[sqlx(type_name = "text")]
pub enum DbMeetingProvider {
    #[sqlx(rename = "TWILIO")]
    Twilio,
    #[sqlx(rename = "TOKBOX")]
    TokBox,
}

impl From<DbMeetingProvider> for MeetingProvider {
    fn from(db: DbMeetingProvider) -> Self {
        match db {
            DbMeetingProvider::Twilio => MeetingProvider::Twilio,
            DbMeetingProvider::TokBox => MeetingProvider::TokBox,
        }
    }
}

impl From<MeetingProvider> for DbMeetingProvider {
    fn from(proto: MeetingProvider) -> Self {
        match proto {
            MeetingProvider::Twilio => DbMeetingProvider::Twilio,
            MeetingProvider::TokBox => DbMeetingProvider::TokBox,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
#[sqlx(type_name = "text", rename_all = "PascalCase")]
pub enum PaymentChannelType {
    SelfPay,
    Insurance,
    InsuranceWithSelfPay,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[sqlx(type_name = "v2.session_info_status_enum")]
pub enum SessionStatus {
    #[sqlx(rename = "EMPTY_ROOM_CREATED")]
    EmptyRoomCreated,
    #[sqlx(rename = "DOCTOR_JOINED")]
    DoctorJoined,
    #[sqlx(rename = "PATIENT_JOINED")]
    PatientJoined,
    #[sqlx(rename = "ALL_PARTICIPATNS_JOINED")]
    AllParticipantsJoined,
    #[sqlx(rename = "ENDED")]
    Ended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
#[sqlx(type_name = "v2.booking_type_enum")]
pub enum BookingTypeEnum {
    #[sqlx(rename = "Instant")]
    Instant,
    #[sqlx(rename = "Schedule")]
    Schedule,
    #[sqlx(rename = "FollowUp")]
    FollowUp,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[sqlx(type_name = "v2.fhir_appointment_status_enum")]
pub enum AppointmentStatusEnum {
    #[sqlx(rename = "PROPOSED")]
    Proposed,
    #[sqlx(rename = "PENDING")]
    Pending,
    #[sqlx(rename = "BOOKED")]
    Booked,
    #[sqlx(rename = "ARRIVED")]
    Arrived,
    #[sqlx(rename = "FULFILLED")]
    Fulfilled,
    #[serde(rename = "CONSULTATION_DONE")]
    #[sqlx(rename = "CONSULTATION_DONE")]
    ConsultationDone,
    #[sqlx(rename = "CANCELLED")]
    Cancelled,
    #[sqlx(rename = "NOSHOW")]
    Noshow,
    #[sqlx(rename = "ENTERED_IN_ERROR")]
    EnteredInError,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[sqlx(type_name = "v2.appointment_type_enum")]
pub enum AppointmentTypeEnum {
    #[sqlx(rename = "ROUTINE")]
    Routine,
    #[sqlx(rename = "WALK_IN")]
    WalkIn,
    #[sqlx(rename = "EMERGENCY")]
    Emergency,
    #[sqlx(rename = "URGENT")]
    Urgent,
}
