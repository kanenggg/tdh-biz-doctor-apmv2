use crate::appointment::hold::model::{
    AppointmentHoldCreated, CreateAppointmentHold, PaymentQuote,
};
use crate::common::tdh_protocol::common::PartialUserIdentity;
use crate::common::tdh_protocol::consultation::{ConsultationEvent, PreSessionMessage};
use crate::common::tdh_protocol::{
    common::PartialUserIdentity as LegacyPatientIdentity, doctor::profile::DoctorProfile,
    iam::user_identity::UserIdentity,
};
use crate::consultation_config::model::ScheduleAvailableConfig;
use crate::infra::event::outbox::enqueue_consultation_event_in_tx;
use deadpool_redis::{Pool as RedisPool, redis::AsyncCommands};
use sqlx::{FromRow, PgPool};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorHoldAvailability {
    pub is_active: bool,
    pub schedule_available: bool,
    pub instant_available: bool,
    pub schedule_config: ScheduleAvailableConfig,
    pub service_config: Option<DoctorServiceConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorServiceConfig {
    pub channels: Vec<String>,
    pub duration_minutes: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum AppointmentHoldRepoError {
    #[error("postgres error: {0}")]
    Postgres(#[from] sqlx::Error),
    #[error("infrastructure error: {0}")]
    Infrastructure(#[from] anyhow::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DoctorHoldProfileError {
    #[error("doctor profile cache unavailable: {0}")]
    Cache(#[from] deadpool_redis::PoolError),
    #[error("doctor profile cache command failed: {0}")]
    Redis(#[from] deadpool_redis::redis::RedisError),
    #[error("doctor profile cache contained invalid JSON: {0}")]
    Deserialize(#[from] serde_json::Error),
}

impl AppointmentHoldRepoError {
    pub fn is_invalid_request(&self) -> bool {
        matches!(self, Self::Postgres(error) if error.as_database_error().is_some_and(|db| db.code().as_deref() == Some("22023")))
    }

    pub fn is_overlap(&self) -> bool {
        matches!(self, Self::Postgres(error) if error.as_database_error().is_some_and(|db| db.code().as_deref() == Some("23P01")))
    }

    pub fn is_unavailable(&self) -> bool {
        matches!(self, Self::Postgres(error) if error.as_database_error().is_some_and(|db| {
            db.code().as_deref() == Some("P0001") || db.message().contains("DOCTOR_NOT_AVAILABLE")
        }))
    }
}

#[async_trait::async_trait]
pub trait AppointmentHoldRepo: Send + Sync {
    async fn create_hold(
        &self,
        patient: &UserIdentity,
        request: &CreateAppointmentHold,
        doctor_account_id: i64,
        doctor_profile_id: i64,
        ttl_seconds: i32,
        created_at: i64,
    ) -> Result<AppointmentHoldCreated, AppointmentHoldRepoError>;

    async fn availability(
        &self,
        doctor_id: uuid::Uuid,
        doctor_account_id: i64,
        doctor_profile_id: i64,
    ) -> Result<Option<DoctorHoldAvailability>, AppointmentHoldRepoError>;
}

#[async_trait::async_trait]
pub trait DoctorHoldProfileRepo: Send + Sync {
    async fn doctor_profile(
        &self,
        doctor_id: i32,
    ) -> Result<Option<DoctorProfile>, DoctorHoldProfileError>;
}

pub struct AppointmentHoldPsql {
    pool: PgPool,
    consultation_topic: String,
}

impl AppointmentHoldPsql {
    pub fn new(pool: PgPool, consultation_topic: impl Into<String>) -> Self {
        Self {
            pool,
            consultation_topic: consultation_topic.into(),
        }
    }
}

#[async_trait::async_trait]
impl AppointmentHoldRepo for AppointmentHoldPsql {
    async fn create_hold(
        &self,
        patient: &UserIdentity,
        request: &CreateAppointmentHold,
        doctor_account_id: i64,
        doctor_profile_id: i64,
        ttl_seconds: i32,
        created_at: i64,
    ) -> Result<AppointmentHoldCreated, AppointmentHoldRepoError> {
        let start = jiff::Timestamp::from_second(request.timeslot.start).map_err(|error| {
            AppointmentHoldRepoError::Infrastructure(anyhow::anyhow!("invalid Hold start: {error}"))
        })?;
        let mut tx = self.pool.begin().await?;
        let booking_id: String = sqlx::query_scalar(
            "SELECT booking_id FROM v2.create_appointment_hold($1,$2,$3,$4,$5,$6,$7,$8,$9::v2.booking_type_enum,$10::v2.consultation_type_enum,$11::timestamptz,$12,$13,true)",
        )
        .bind(patient.account_id as i32).bind(patient.user_profile_id as i32)
        .bind(request.doctor_id).bind(doctor_account_id as i32).bind(doctor_profile_id as i32)
        .bind(request.biz_unit_id).bind(request.biz_center_id).bind(patient.tenant_id as i32)
        .bind(String::from(request.booking_type.clone())).bind(String::from(request.consultation_channel.clone()))
        .bind(jiff_sqlx::Timestamp::from(start)).bind(ttl_seconds).bind(request.timeslot.duration)
        .fetch_one(&mut *tx).await?;
        let patient_intake = serde_json::to_string(&request.patient_intake).map_err(|error| {
            AppointmentHoldRepoError::Infrastructure(anyhow::anyhow!(
                "could not serialize Appointment Hold patient intake: {error}"
            ))
        })?;
        // This remains inside the Hold/outbox transaction: a visible Hold can
        // never exist without the canonical prescreen it promises to transfer.
        sqlx::query_scalar::<_, i32>("SELECT v2.attach_hold_prescreen($1, $2, 'RAW_JSON')")
            .bind(&booking_id)
            .bind(patient_intake)
            .fetch_one(&mut *tx)
            .await?;
        let event = legacy_v1_timeslot_reserved_event(&booking_id, patient, request, created_at);
        enqueue_consultation_event_in_tx(&mut tx, &self.consultation_topic, &event).await?;
        let (amount, currency, effective_service_config_version): (String, String, i64) = sqlx::query_as(
                "SELECT quoted_amount::text, quoted_currency, quoted_service_config_version FROM v2.appointment_hold WHERE booking_id = $1",
            )
            .bind(&booking_id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(AppointmentHoldCreated {
            booking_id,
            payment_quote: PaymentQuote {
                amount,
                currency,
                effective_service_config_version,
            },
        })
    }

    async fn availability(
        &self,
        doctor_id: uuid::Uuid,
        doctor_account_id: i64,
        doctor_profile_id: i64,
    ) -> Result<Option<DoctorHoldAvailability>, AppointmentHoldRepoError> {
        let row = sqlx::query_as::<_, (bool, bool, bool, Option<serde_json::Value>, Option<Vec<String>>, Option<i32>)>(
            "SELECT dip.is_active, COALESCE(dcc.schedule_available,false), COALESCE(dcc.instant_available,false), dcc.schedule_config, dscp.channels, dscp.duration_minutes FROM v2.doctor_info_projection dip LEFT JOIN v2.doctor_consultation_config dcc ON dcc.doctor_id=dip.doctor_id LEFT JOIN v2.doctor_service_config_projection dscp ON dscp.doctor_id=dip.doctor_id WHERE dip.doctor_id=$1 AND dip.doctor_account_id=$2 AND dip.doctor_profile_id=$3"
        ).bind(doctor_id).bind(doctor_account_id).bind(doctor_profile_id).fetch_optional(&self.pool).await?;
        row.map(
            |(
                is_active,
                schedule_available,
                instant_available,
                schedule_config,
                channels,
                duration_minutes,
            )| {
                Ok(DoctorHoldAvailability {
                    is_active,
                    schedule_available,
                    instant_available,
                    schedule_config: schedule_config
                        .map(serde_json::from_value)
                        .transpose()?
                        .unwrap_or_default(),
                    service_config: match (channels, duration_minutes) {
                        (Some(channels), Some(duration_minutes)) => Some(DoctorServiceConfig {
                            channels,
                            duration_minutes,
                        }),
                        _ => None,
                    },
                })
            },
        )
        .transpose()
        .map_err(AppointmentHoldRepoError::Infrastructure)
    }
}

pub struct DoctorHoldProfileCache {
    redis_pool: RedisPool,
}
impl DoctorHoldProfileCache {
    pub fn new(redis_pool: RedisPool) -> Self {
        Self { redis_pool }
    }
}
#[async_trait::async_trait]
impl DoctorHoldProfileRepo for DoctorHoldProfileCache {
    async fn doctor_profile(
        &self,
        doctor_id: i32,
    ) -> Result<Option<DoctorProfile>, DoctorHoldProfileError> {
        let mut connection = self.redis_pool.get().await?;
        let raw: Option<String> = connection
            .get(format!("doctor:{doctor_id}:profile"))
            .await?;
        raw.map(|value| serde_json::from_str(&value).map_err(DoctorHoldProfileError::from))
            .transpose()
    }
}

fn legacy_v1_timeslot_reserved_event(
    booking_id: &str,
    patient: &UserIdentity,
    request: &CreateAppointmentHold,
    created_at: i64,
) -> ConsultationEvent {
    ConsultationEvent::PreSessionMessage(PreSessionMessage::TimeslotReserved {
        booking_id: booking_id.to_string(),
        patient_identity: LegacyPatientIdentity {
            account_id: patient.account_id,
            user_profile_id: patient.user_profile_id,
            tenant_id: patient.tenant_id,
            oidc_user_id: patient.oidc_user_id.clone(),
        },
        doctor_id: request.doctor_id,
        biz_unit_id: request.biz_unit_id,
        reserved_from: request.timeslot.start,
        reservation_duration_sec: request.timeslot.duration,
        consultation_channel: request.consultation_channel.clone(),
        reserved_at: created_at,
    })
}

#[derive(Debug, thiserror::Error)]
pub enum BookingRepoError {
    #[error("booking cannot be cancelled in its current state")]
    CannotCancel,
    #[error(transparent)]
    Unexpected(#[from] anyhow::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct BookingStateRow {
    pub booking_id: String,
    pub patient_account_id: i32,
    pub patient_profile_id: i32,
    pub tenant_id: i32,
    pub doctor_id: i32,
    pub biz_unit_id: i32,
    pub reservation_status: String,
    pub appointment_status: Option<String>,
    pub reserved_until: i64,
    pub appointment_start: i64,
    pub appointment_end: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct CancelReservedBookingRow {
    pub booking_id: String,
    pub patient_account_id: i32,
    pub patient_profile_id: i32,
    pub tenant_id: i32,
    pub doctor_id: i32,
    pub biz_unit_id: i32,
    pub reservation_status: String,
    pub appointment_status: Option<String>,
    pub cancelled_at: i64,
    pub state_changed: bool,
}

#[async_trait::async_trait]
pub trait BookingRepo: Send + Sync {
    async fn get_booking_state(
        &self,
        booking_id: &str,
    ) -> Result<Option<BookingStateRow>, BookingRepoError>;

    async fn cancel_reserved_booking(
        &self,
        booking_id: &str,
    ) -> Result<Option<CancelReservedBookingRow>, BookingRepoError>;
}

pub struct BookingRepoPsql {
    pool: PgPool,
    consultation_topic: Option<String>,
}

impl BookingRepoPsql {
    pub fn new(pool: PgPool, consultation_topic: impl Into<String>) -> Self {
        Self::with_v1_topic(pool, Some(consultation_topic.into()))
    }

    pub fn with_v1_topic(pool: PgPool, consultation_topic: Option<String>) -> Self {
        Self {
            pool,
            consultation_topic,
        }
    }
}

#[async_trait::async_trait]
impl BookingRepo for BookingRepoPsql {
    async fn get_booking_state(
        &self,
        booking_id: &str,
    ) -> Result<Option<BookingStateRow>, BookingRepoError> {
        sqlx::query_as::<_, BookingStateRow>(
            r#"
            SELECT * FROM v2.get_booking_state($1)
            "#,
        )
        .bind(booking_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            BookingRepoError::Unexpected(anyhow::anyhow!("Failed to get booking state: {e}"))
        })
    }

    async fn cancel_reserved_booking(
        &self,
        booking_id: &str,
    ) -> Result<Option<CancelReservedBookingRow>, BookingRepoError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            BookingRepoError::Unexpected(anyhow::anyhow!(
                "Failed to begin cancel reservation transaction: {e}"
            ))
        })?;

        let row = sqlx::query_as::<_, CancelReservedBookingRow>(
            r#"
            SELECT * FROM v2.cancel_reserved_booking($1)
            "#,
        )
        .bind(booking_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(map_cancel_reserved_booking_error)?;

        if let Some(row) = &row {
            if row.state_changed {
                let event = legacy_v1_hold_terminal_event(row);
                let consultation_topic = self.consultation_topic.as_deref().ok_or_else(|| {
                    BookingRepoError::Unexpected(anyhow::anyhow!(
                        "Consultation V1 event publishing is disabled by the rollout mode"
                    ))
                })?;
                enqueue_consultation_event_in_tx(&mut tx, consultation_topic, &event)
                    .await
                    .map_err(|e| {
                        BookingRepoError::Unexpected(anyhow::anyhow!(
                            "Failed to enqueue reservation cancelled event: {e}"
                        ))
                    })?;
            }
        }

        tx.commit().await.map_err(|e| {
            BookingRepoError::Unexpected(anyhow::anyhow!(
                "Failed to commit cancel reservation transaction: {e}"
            ))
        })?;

        Ok(row)
    }
}

/// Explicit V1 wire adapter: domain Hold release retains the legacy
/// `ReservationCancelled` discriminator until consumers are versioned.
fn legacy_v1_reservation_cancelled_event(row: &CancelReservedBookingRow) -> ConsultationEvent {
    ConsultationEvent::PreSessionMessage(PreSessionMessage::ReservationCancelled {
        booking_id: row.booking_id.clone(),
        patient_identity: PartialUserIdentity {
            account_id: row.patient_account_id as u64,
            user_profile_id: row.patient_profile_id as u64,
            tenant_id: row.tenant_id as u32,
            oidc_user_id: None,
        },
        doctor_id: row.doctor_id,
        biz_unit_id: row.biz_unit_id,
        cancelled_at: row.cancelled_at,
    })
}

fn legacy_v1_hold_terminal_event(row: &CancelReservedBookingRow) -> ConsultationEvent {
    if row.reservation_status == "RESERVE_EXPIRED" {
        ConsultationEvent::PreSessionMessage(PreSessionMessage::ReservationExpired {
            booking_id: row.booking_id.clone(),
            patient_identity: PartialUserIdentity {
                account_id: row.patient_account_id as u64,
                user_profile_id: row.patient_profile_id as u64,
                tenant_id: row.tenant_id as u32,
                oidc_user_id: None,
            },
            doctor_id: row.doctor_id,
            biz_unit_id: row.biz_unit_id,
            cancelled_at: row.cancelled_at,
        })
    } else {
        legacy_v1_reservation_cancelled_event(row)
    }
}

fn map_cancel_reserved_booking_error(error: sqlx::Error) -> BookingRepoError {
    if is_cannot_cancel_reserved_booking_error(&error) {
        BookingRepoError::CannotCancel
    } else {
        BookingRepoError::Unexpected(anyhow::anyhow!(
            "Failed to cancel reserved booking: {error}"
        ))
    }
}

fn is_cannot_cancel_reserved_booking_error(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .is_some_and(|database_error| database_error.code().as_deref() == Some("P1006"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cancelled_booking(state_changed: bool) -> CancelReservedBookingRow {
        CancelReservedBookingRow {
            booking_id: "booking-1".to_string(),
            patient_account_id: 1,
            patient_profile_id: 2,
            tenant_id: 3,
            doctor_id: 4,
            biz_unit_id: 5,
            reservation_status: "CANCELLED".to_string(),
            appointment_status: None,
            cancelled_at: 4_000,
            state_changed,
        }
    }

    #[test]
    fn cancel_reserved_booking_uses_one_transaction_for_state_change_and_outbox() {
        let source = include_str!("repo.rs");

        assert!(source.contains(".begin()"));
        assert!(source.contains("SELECT * FROM v2.cancel_reserved_booking($1)"));
        assert!(source.contains("if row.state_changed"));
        assert!(source.contains("enqueue_consultation_event_in_tx"));
        assert!(source.contains(".commit()"));
    }

    #[test]
    fn builds_reservation_cancelled_event_payload_for_outbox() {
        let row = cancelled_booking(true);

        let event = legacy_v1_reservation_cancelled_event(&row);

        match event {
            ConsultationEvent::PreSessionMessage(PreSessionMessage::ReservationCancelled {
                booking_id,
                patient_identity,
                doctor_id,
                biz_unit_id,
                cancelled_at,
            }) => {
                assert_eq!(booking_id, "booking-1");
                assert_eq!(patient_identity.account_id, 1);
                assert_eq!(patient_identity.user_profile_id, 2);
                assert_eq!(patient_identity.tenant_id, 3);
                assert!(patient_identity.oidc_user_id.is_none());
                assert_eq!(doctor_id, 4);
                assert_eq!(biz_unit_id, 5);
                assert_eq!(cancelled_at, 4_000);
            }
            event => panic!("unexpected event: {event:?}"),
        }
    }
}
