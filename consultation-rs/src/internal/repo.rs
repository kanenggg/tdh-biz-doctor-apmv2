use crate::common::tdh_protocol::{
    appointment::AppointmentStatus,
    consultation::{BookingType, ConsultationChannel},
    internal::{
        CreateAppointmentRequest, CreateAppointmentResult, CreateConfirmedInstantAppointmentRequest,
    },
};
use jiff::Timestamp;
use sqlx::PgPool;

pub struct InternalRepo {
    pg_pool: PgPool,
}

impl InternalRepo {
    pub fn new(pg_pool: PgPool) -> Self {
        Self { pg_pool }
    }

    // NOTE: CreateConfirmedInstantAppointmentRequest in tdh-protocol does not
    // carry a payment_tx_id field. v2.create_confirmed_appointment continues
    // to rely on the column's DEFAULT 0 for now. Follow-up: extend the
    // protocol request type and plumb a real value through here.
    pub async fn add_confirmed_appointment(
        &self,
        req: CreateConfirmedInstantAppointmentRequest,
    ) -> Result<i64, anyhow::Error> {
        let consult_duration_minutes = req.consult_duration.unwrap_or(20);
        let now = Timestamp::now();
        let appointment_start = jiff_sqlx::Timestamp::from(now);
        let appointment_end_ts = now
            .saturating_add(jiff::ToSpan::minutes(consult_duration_minutes))
            .map_err(|e| anyhow::anyhow!("Failed to calculate appointment end time: {e}"))?;
        let appointment_end = jiff_sqlx::Timestamp::from(appointment_end_ts);

        let booking_type_str = booking_type_str(&req.booking_type);
        let consultation_channel_str = consultation_channel_str(&req.consultation_channel);
        let prescreen_data = serde_json::to_string(&req.prescreen)?;

        let booking_id: String = sqlx::query_scalar(
            r#"
            SELECT booking_id
            FROM v2.create_confirmed_appointment(
                $1::integer,
                $2::integer,
                $3::integer,
                $4::integer,
                $5::integer,
                $6::integer,
                $7::integer,
                $8::integer,
                $9::v2.booking_type_enum,
                $10::v2.consultation_type_enum,
                $11::timestamptz,
                $12::timestamptz,
                $13::text,
                $14::varchar,
                $15::varchar,
                $16::jsonb
            )
            "#,
        )
        .bind(req.patient_id.account_id as i32)
        .bind(req.patient_id.user_profile_id as i32)
        .bind(req.doctor_id.account_id as i32)
        .bind(req.doctor_id.account_id as i32)
        .bind(req.doctor_id.user_profile_id as i32)
        .bind(req.biz_unit_id)
        .bind(req.biz_center_id)
        .bind(req.tenant_id)
        .bind(booking_type_str)
        .bind(consultation_channel_str)
        .bind(appointment_start)
        .bind(appointment_end)
        .bind(prescreen_data)
        .bind("RAW_JSON")
        .bind(req.parent_appointment_id)
        .bind(sqlx::types::Json(req.payment_channels))
        .fetch_one(&self.pg_pool)
        .await?;

        Ok(booking_id.parse::<i64>()?)
    }

    pub async fn create_appointment(
        &self,
        req: CreateAppointmentRequest,
    ) -> Result<CreateAppointmentResult, anyhow::Error> {
        let appointment_no = req.appointment_no.clone();
        let appointment_start = jiff::Timestamp::from_second(req.appointment_start)
            .map_err(|e| anyhow::anyhow!("Invalid appointment_start timestamp: {e}"))?;
        let appointment_end = jiff::Timestamp::from_second(req.appointment_end)
            .map_err(|e| anyhow::anyhow!("Invalid appointment_end timestamp: {e}"))?;

        let appointment_start_sqlx = jiff_sqlx::Timestamp::from(appointment_start);
        let appointment_end_sqlx = jiff_sqlx::Timestamp::from(appointment_end);

        let booking_type_str = booking_type_str(&req.booking_type);
        let consultation_channel_str = consultation_channel_str(&req.consultation_channel);
        let appointment_status_str = appointment_status_str(&req.appointment_status);
        let prescreen_data = match &req.prescreen {
            Some(prescreen) => serde_json::to_string(prescreen)?,
            None => "{}".to_string(),
        };
        let payment_channels = req
            .payment_channels
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| anyhow::anyhow!("Failed to serialize payment channels: {e}"))?;

        let row: (String, String) = sqlx::query_as(
            r#"
            SELECT booking_id, appointment_id
            FROM v2.create_appointment_internal(
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7,
                $8::v2.booking_type_enum,
                $9::v2.consultation_type_enum,
                $10::timestamptz,
                $11::timestamptz,
                $12::v2.fhir_appointment_status_enum,
                $13::bigint,
                $14::varchar,
                $15::jsonb,
                $16::varchar,
                $17::text,
                $18::varchar,
                $19::varchar
            )
            "#,
        )
        .bind(req.patient_id.account_id as i32)
        .bind(req.patient_id.user_profile_id as i32)
        .bind(req.doctor_id.account_id as i32)
        .bind(req.doctor_id.user_profile_id as i32)
        .bind(req.biz_unit_id)
        .bind(req.biz_center_id)
        .bind(req.tenant_id)
        .bind(booking_type_str)
        .bind(consultation_channel_str)
        .bind(appointment_start_sqlx)
        .bind(appointment_end_sqlx)
        .bind(appointment_status_str)
        .bind(req.payment_tx_id.unwrap_or(0))
        .bind(req.payment_tx_ref_id)
        .bind(payment_channels)
        .bind(req.parent_appointment_id)
        .bind(prescreen_data)
        .bind("RAW_JSON")
        .bind(req.appointment_no)
        .fetch_one(&self.pg_pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to create appointment: {e}"))?;

        let ref_code = appointment_no.clone().unwrap_or_else(|| row.0.clone());

        Ok(CreateAppointmentResult {
            booking_id: row.0,
            appointment_id: row.1,
            appointment_no,
            ref_code,
        })
    }
}

fn booking_type_str(booking_type: &BookingType) -> &'static str {
    match booking_type {
        BookingType::Instant => "Instant",
        BookingType::Schedule => "Schedule",
    }
}

fn consultation_channel_str(channel: &ConsultationChannel) -> &'static str {
    match channel {
        ConsultationChannel::Video => "video",
        ConsultationChannel::Voice => "voice",
        ConsultationChannel::Chat => "chat",
    }
}

fn appointment_status_str(status: &AppointmentStatus) -> &'static str {
    match status {
        AppointmentStatus::Proposed => "PROPOSED",
        AppointmentStatus::Pending => "PENDING",
        AppointmentStatus::Booked => "BOOKED",
        AppointmentStatus::Arrived => "ARRIVED",
        AppointmentStatus::Fulfilled => "FULFILLED",
        AppointmentStatus::Cancelled => "CANCELLED",
        AppointmentStatus::Noshow => "NOSHOW",
        AppointmentStatus::EnteredInError => "ENTERED_IN_ERROR",
    }
}
