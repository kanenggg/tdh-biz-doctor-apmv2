use crate::common::tdh_protocol::{
    appointment::v2::payment_transaction::PaymentChannel, common::meeting_provider::MeetingProvider,
};
use crate::repo::enums::{AppointmentStatusEnum, ConsultationChannelEnum, DbMeetingProvider};
use sqlx::{FromRow, types::Json};

#[derive(Clone, Debug, FromRow)]
pub struct DbConsultationSession {
    pub appointment_id: String,
    pub session_provider_name: DbMeetingProvider,
    pub appointment_status: AppointmentStatusEnum,
    pub session_data: Option<Json<serde_json::Value>>,
    pub patient_profile_id: i64,
    pub doctor_profile_id: i64,
    pub consultation_start_time: i64,
    pub consultation_end_time: i64,
    pub consultation_channel: ConsultationChannelEnum,
    pub payment_channels: Option<Json<Vec<PaymentChannel>>>,
    pub is_facial_verified: bool,
}

pub struct SessionDetails {
    pub appointment_id: String,
    pub booking_id: String,
    pub patient_account_id: u64,
    pub patient_profile_id: u64,
    pub tenant_id: u32,
    pub doctor_id: i32,
    pub doctor_profile_id: i64,
    pub session_provider: MeetingProvider,
    pub session_chat_id: Option<String>,
}

#[derive(FromRow)]
pub struct DbSessionDetails {
    pub appointment_id: String,
    pub booking_id: String,
    pub patient_account_id: i32,
    pub patient_profile_id: i32,
    pub tenant_id: i32,
    pub doctor_id: i32,
    pub doctor_profile_id: i32,
    pub session_provider: String,
    pub session_chat_id: Option<String>,
}

impl From<DbSessionDetails> for SessionDetails {
    fn from(db: DbSessionDetails) -> Self {
        let session_provider = match db.session_provider.as_str() {
            "TWILIO" => MeetingProvider::Twilio,
            "TOKBOX" => MeetingProvider::TokBox,
            _ => MeetingProvider::Twilio,
        };

        Self {
            appointment_id: db.appointment_id,
            booking_id: db.booking_id,
            patient_account_id: db.patient_account_id as u64,
            patient_profile_id: db.patient_profile_id as u64,
            tenant_id: db.tenant_id as u32,
            doctor_id: db.doctor_id,
            doctor_profile_id: db.doctor_profile_id as i64,
            session_provider,
            session_chat_id: db.session_chat_id,
        }
    }
}
