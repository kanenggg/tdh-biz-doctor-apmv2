//! Canonical Appointment Hold creation.
//!
//! Public HTTP and protocol payloads still use `booking`/`ReserveRequest` at
//! their compatibility boundary.  The domain operation here is a Hold: a
//! pre-booking claim on doctor occupancy that can later be released or expire.

use crate::appointment::hold::model::{AppointmentHoldCreated, CreateAppointmentHold};
use crate::appointment::hold::repo::{
    AppointmentHoldRepo, AppointmentHoldRepoError, DoctorHoldAvailability, DoctorHoldProfileError,
    DoctorHoldProfileRepo,
};
use crate::common::tdh_protocol::consultation::BookingType;
use crate::common::tdh_protocol::iam::user_identity::UserIdentity;
use crate::doctor_timeslot::get_timeslot::service::timeslot_within_schedule;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum HoldError {
    #[error("Invalid Appointment Hold request: {0}")]
    InvalidRequest(String),
    #[error("Doctor is not available for this Appointment Hold")]
    DoctorNotAvailable,
    #[error("Appointment Hold overlaps an existing Doctor Occupancy")]
    SlotAlreadyBooked,
    #[error("Appointment Hold storage error: {0}")]
    Database(#[from] AppointmentHoldRepoError),
    #[error("Appointment Hold profile lookup failed: {0}")]
    Profile(#[from] DoctorHoldProfileError),
}

/// The canonical owner of pre-booking Hold creation.
#[derive(Clone)]
pub struct AppointmentHoldService {
    repo: Arc<dyn AppointmentHoldRepo>,
    doctor_profiles: Arc<dyn DoctorHoldProfileRepo>,
    ttl_seconds: i32,
}

impl AppointmentHoldService {
    pub fn new(
        repo: Arc<dyn AppointmentHoldRepo>,
        doctor_profiles: Arc<dyn DoctorHoldProfileRepo>,
        ttl_seconds: i32,
    ) -> Self {
        Self {
            repo,
            doctor_profiles,
            ttl_seconds,
        }
    }

    pub async fn create_hold(
        &self,
        patient: UserIdentity,
        request: CreateAppointmentHold,
    ) -> Result<AppointmentHoldCreated, HoldError> {
        validate_hold_timeslot(&request)?;

        let now = jiff::Timestamp::now();
        let doctor_profile = self
            .doctor_profiles
            .doctor_profile(request.doctor_id)
            .await
            .map_err(HoldError::Profile)?
            .ok_or(HoldError::DoctorNotAvailable)?;

        let doctor_id = uuid::Uuid::parse_str(&doctor_profile.doctor_id)
            .map_err(|_| HoldError::DoctorNotAvailable)?;
        let availability = self
            .repo
            .availability(
                doctor_id,
                doctor_profile.iam_account_id,
                doctor_profile.iam_profile_id,
            )
            .await
            .map_err(HoldError::Database)?
            .ok_or(HoldError::DoctorNotAvailable)?;

        if !is_hold_available(&availability, &request)? {
            return Err(HoldError::DoctorNotAvailable);
        }

        self.repo
            .create_hold(
                &patient,
                &request,
                doctor_profile.iam_account_id,
                doctor_profile.iam_profile_id,
                self.ttl_seconds,
                now.as_second(),
            )
            .await
            .map_err(map_create_hold_error)
    }
}

fn is_hold_available(
    availability: &DoctorHoldAvailability,
    request: &CreateAppointmentHold,
) -> Result<bool, HoldError> {
    if !availability.is_active {
        return Ok(false);
    }
    let is_instant = matches!(request.booking_type, BookingType::Instant);
    if (is_instant && !availability.instant_available)
        || (!is_instant && !availability.schedule_available)
    {
        return Ok(false);
    }
    match &availability.service_config {
        Some(config) => {
            let channel: String = request.consultation_channel.clone().into();
            if !config
                .channels
                .iter()
                .any(|supported| supported == &channel)
                || request.timeslot.duration != i64::from(config.duration_minutes) * 60
            {
                return Ok(false);
            }
        }
        None => return Ok(false),
    }

    if is_instant {
        Ok(true)
    } else {
        timeslot_within_schedule(
            &availability.schedule_config,
            request.timeslot.start,
            request.timeslot.end,
        )
        .map_err(|error| HoldError::InvalidRequest(error.to_string()))
    }
}

fn validate_hold_timeslot(request: &CreateAppointmentHold) -> Result<(), HoldError> {
    let duration = request
        .timeslot
        .end
        .checked_sub(request.timeslot.start)
        .ok_or_else(|| HoldError::InvalidRequest("timeslot end must be after start".to_string()))?;

    if duration <= 0 {
        return Err(HoldError::InvalidRequest(
            "timeslot end must be after start".to_string(),
        ));
    }
    if request.timeslot.duration <= 0 {
        return Err(HoldError::InvalidRequest(
            "timeslot duration must be positive".to_string(),
        ));
    }
    if request.timeslot.duration != duration {
        return Err(HoldError::InvalidRequest(
            "timeslot duration must match end minus start".to_string(),
        ));
    }

    jiff::Timestamp::from_second(request.timeslot.start).map_err(|error| {
        HoldError::InvalidRequest(format!("invalid timeslot start timestamp: {error}"))
    })?;
    jiff::Timestamp::from_second(request.timeslot.end).map_err(|error| {
        HoldError::InvalidRequest(format!("invalid timeslot end timestamp: {error}"))
    })?;
    Ok(())
}

fn map_create_hold_error(error: AppointmentHoldRepoError) -> HoldError {
    if error.is_invalid_request() {
        HoldError::InvalidRequest("invalid Appointment Hold request".to_string())
    } else if error.is_overlap() {
        HoldError::SlotAlreadyBooked
    } else if error.is_unavailable() {
        HoldError::DoctorNotAvailable
    } else {
        HoldError::Database(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::appointment::hold::repo::DoctorServiceConfig;
    use crate::common::tdh_protocol::appointment::reserve::{PatientPrescreen, Timeslot};
    use crate::common::tdh_protocol::consultation::{BookingType, ConsultationChannel};
    use crate::consultation_config::model::ScheduleAvailableConfig;

    fn request() -> CreateAppointmentHold {
        CreateAppointmentHold {
            doctor_id: 1,
            biz_unit_id: 1,
            biz_center_id: 1,
            patient_intake: PatientPrescreen {
                symptom: String::new(),
                symptom_duration: 1,
                symptom_duration_unit: "day".into(),
                attachments: vec![],
                allergies: vec![],
            },
            consultation_channel: ConsultationChannel::Video,
            timeslot: Timeslot {
                start: 1_781_762_400,
                end: 1_781_764_200,
                duration: 1_800,
            },
            booking_type: BookingType::Instant,
            trace_id: None,
        }
    }

    #[test]
    fn instant_hold_does_not_require_a_schedule_window() {
        let availability = DoctorHoldAvailability {
            is_active: true,
            schedule_available: false,
            instant_available: true,
            schedule_config: ScheduleAvailableConfig::default(),
            service_config: Some(DoctorServiceConfig {
                channels: vec!["video".into()],
                duration_minutes: 30,
            }),
        };
        assert!(is_hold_available(&availability, &request()).expect("valid instant Hold"));
    }
}
