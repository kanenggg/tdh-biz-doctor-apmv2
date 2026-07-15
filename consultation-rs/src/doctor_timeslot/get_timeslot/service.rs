use std::sync::Arc;

use jiff::{Timestamp, civil::Date, tz::TimeZone};
use uuid::Uuid;

use crate::common::tdh_protocol::consultation::ConsultationChannel;
use crate::consultation_config::model::{ScheduleAvailableConfig, TimePeriod};
use crate::doctor_timeslot::get_timeslot::model::{
    AvailableDoctor, AvailableDoctorsResponse, AvailableTimeslot, AvailableTimeslotResponse,
};
use crate::doctor_timeslot::get_timeslot::repo::{
    DoctorScheduleCandidate, DoctorTimeslotIdentity, GetDoctorTimeslotRepo, ReservedWindow,
};

#[derive(Debug, thiserror::Error)]
pub enum GetDoctorTimeslotError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("doctor identity is not provisioned for this account/profile")]
    DoctorIdentityNotProvisioned,
    #[error("doctor identity is inactive")]
    DoctorInactive,
    #[error("repository error: {0}")]
    Repository(#[from] anyhow::Error),
}

#[derive(Clone)]
pub struct GetDoctorTimeslotService {
    repo: Arc<dyn GetDoctorTimeslotRepo>,
    slot_duration_seconds: i64,
    require_v2_snapshot: bool,
}

impl GetDoctorTimeslotService {
    pub fn new(repo: Arc<dyn GetDoctorTimeslotRepo>, slot_duration_seconds: i64) -> Self {
        Self::new_with_v2_snapshot(repo, slot_duration_seconds, false)
    }

    pub fn new_with_v2_snapshot(
        repo: Arc<dyn GetDoctorTimeslotRepo>,
        slot_duration_seconds: i64,
        require_v2_snapshot: bool,
    ) -> Self {
        Self {
            repo,
            slot_duration_seconds,
            require_v2_snapshot,
        }
    }

    pub async fn get_available_timeslots(
        &self,
        doctor_account_id: i64,
        doctor_profile_id: i64,
        from_datetime: &str,
        to_datetime: &str,
        consultation_channel: Option<ConsultationChannel>,
    ) -> Result<AvailableTimeslotResponse, GetDoctorTimeslotError> {
        let from = parse_epoch_second(from_datetime)?;
        let to = parse_epoch_second(to_datetime)?;
        if from >= to {
            return Err(GetDoctorTimeslotError::InvalidRequest(
                "fromDatetime must be before toDatetime".to_string(),
            ));
        }

        let identity = self
            .repo
            .resolve_doctor_identity(doctor_account_id, doctor_profile_id)
            .await?
            .ok_or(GetDoctorTimeslotError::DoctorIdentityNotProvisioned)?;

        if !identity.is_active {
            return Err(GetDoctorTimeslotError::DoctorInactive);
        }

        let snapshot = self.repo.get_config_snapshot(identity.doctor_id).await?;
        if !snapshot.schedule_available {
            return Ok(AvailableTimeslotResponse {
                doctor_id: identity.doctor_id,
                timeslots: Vec::new(),
            });
        }

        let channel = consultation_channel.unwrap_or(ConsultationChannel::Video);
        let Some(slot_duration_seconds) = projected_slot_duration_seconds(
            &snapshot.service_config,
            None,
            self.slot_duration_seconds,
            self.require_v2_snapshot,
        ) else {
            return Ok(AvailableTimeslotResponse {
                doctor_id: identity.doctor_id,
                timeslots: Vec::new(),
            });
        };
        if !supports_channel(
            snapshot.service_config.as_ref(),
            &channel,
            self.require_v2_snapshot,
        ) {
            return Ok(AvailableTimeslotResponse {
                doctor_id: identity.doctor_id,
                timeslots: Vec::new(),
            });
        }

        let reserved = self
            .repo
            .list_reserved_windows(identity.doctor_profile_id, from, to)
            .await?;
        let timeslots = generate_available_timeslots(
            identity.doctor_id,
            &snapshot.schedule_config,
            &reserved,
            from,
            to,
            slot_duration_seconds,
            channel,
        )?;

        Ok(AvailableTimeslotResponse {
            doctor_id: identity.doctor_id,
            timeslots,
        })
    }

    pub async fn search_available_doctors(
        &self,
        date: &str,
        timezone: &str,
        consultation_channel: Option<ConsultationChannel>,
    ) -> Result<AvailableDoctorsResponse, GetDoctorTimeslotError> {
        let (from, to) = local_date_epoch_range(date, timezone)?;
        let channel = consultation_channel.unwrap_or(ConsultationChannel::Video);
        let candidates = self.repo.list_schedule_available_doctors().await?;
        let mut doctors = Vec::new();

        for candidate in candidates {
            if !candidate.identity.is_active || !candidate.schedule_available {
                continue;
            }
            let Some(slot_duration_seconds) = projected_slot_duration_seconds(
                &candidate.service_config,
                None,
                self.slot_duration_seconds,
                self.require_v2_snapshot,
            ) else {
                continue;
            };
            if !supports_channel(
                candidate.service_config.as_ref(),
                &channel,
                self.require_v2_snapshot,
            ) {
                continue;
            }

            let reserved = self
                .repo
                .list_reserved_windows(candidate.identity.doctor_profile_id, from, to)
                .await?;
            let timeslots = candidate_timeslots(
                &candidate,
                &reserved,
                from,
                to,
                slot_duration_seconds,
                channel.clone(),
            )?;

            if let Some(first) = timeslots.first() {
                doctors.push(AvailableDoctor {
                    doctor_id: candidate.identity.doctor_id,
                    doctor_account_id: candidate.identity.doctor_account_id,
                    doctor_profile_id: candidate.identity.doctor_profile_id,
                    available_timeslot_count: timeslots.len(),
                    next_available_timeslot_id: Some(first.timeslot_id.clone()),
                });
            }
        }

        Ok(AvailableDoctorsResponse {
            date: date.to_string(),
            timezone: timezone.to_string(),
            doctors,
        })
    }

    pub async fn get_scheduled_availability_by_doctor_date(
        &self,
        doctor_id: Uuid,
        date: &str,
        timezone: &str,
        consultation_channel: Option<ConsultationChannel>,
    ) -> Result<AvailableTimeslotResponse, GetDoctorTimeslotError> {
        let (from, to) = local_date_epoch_range(date, timezone)?;
        let identity = self
            .repo
            .resolve_doctor_identity_by_doctor_id(doctor_id)
            .await?
            .ok_or(GetDoctorTimeslotError::DoctorIdentityNotProvisioned)?;

        self.get_available_timeslots_for_identity(identity, from, to, consultation_channel)
            .await
    }

    async fn get_available_timeslots_for_identity(
        &self,
        identity: DoctorTimeslotIdentity,
        from: i64,
        to: i64,
        consultation_channel: Option<ConsultationChannel>,
    ) -> Result<AvailableTimeslotResponse, GetDoctorTimeslotError> {
        if !identity.is_active {
            return Err(GetDoctorTimeslotError::DoctorInactive);
        }

        let snapshot = self.repo.get_config_snapshot(identity.doctor_id).await?;
        if !snapshot.schedule_available {
            return Ok(AvailableTimeslotResponse {
                doctor_id: identity.doctor_id,
                timeslots: Vec::new(),
            });
        }

        let channel = consultation_channel.unwrap_or(ConsultationChannel::Video);
        let Some(slot_duration_seconds) = projected_slot_duration_seconds(
            &snapshot.service_config,
            None,
            self.slot_duration_seconds,
            self.require_v2_snapshot,
        ) else {
            return Ok(AvailableTimeslotResponse {
                doctor_id: identity.doctor_id,
                timeslots: Vec::new(),
            });
        };
        if !supports_channel(
            snapshot.service_config.as_ref(),
            &channel,
            self.require_v2_snapshot,
        ) {
            return Ok(AvailableTimeslotResponse {
                doctor_id: identity.doctor_id,
                timeslots: Vec::new(),
            });
        }

        let reserved = self
            .repo
            .list_reserved_windows(identity.doctor_profile_id, from, to)
            .await?;
        let timeslots = generate_available_timeslots(
            identity.doctor_id,
            &snapshot.schedule_config,
            &reserved,
            from,
            to,
            slot_duration_seconds,
            channel,
        )?;

        Ok(AvailableTimeslotResponse {
            doctor_id: identity.doctor_id,
            timeslots,
        })
    }
}

fn parse_epoch_second(dt: &str) -> Result<i64, GetDoctorTimeslotError> {
    let ts: Timestamp = dt.parse().map_err(|e| {
        GetDoctorTimeslotError::InvalidRequest(format!("invalid datetime '{dt}': {e}"))
    })?;
    Ok(ts.as_second())
}

fn candidate_timeslots(
    candidate: &DoctorScheduleCandidate,
    reserved: &[ReservedWindow],
    from_epoch: i64,
    to_epoch: i64,
    slot_duration_seconds: i64,
    consultation_channel: ConsultationChannel,
) -> Result<Vec<AvailableTimeslot>, GetDoctorTimeslotError> {
    generate_available_timeslots(
        candidate.identity.doctor_id,
        &candidate.schedule_config,
        reserved,
        from_epoch,
        to_epoch,
        slot_duration_seconds,
        consultation_channel,
    )
}

fn supports_channel(
    service_config: Option<
        &crate::doctor_timeslot::get_timeslot::repo::DoctorServiceConfigSnapshot,
    >,
    channel: &ConsultationChannel,
    require_v2_snapshot: bool,
) -> bool {
    match service_config {
        Some(config) => config
            .channels
            .iter()
            .any(|value| value == &String::from(channel.clone())),
        None => !require_v2_snapshot,
    }
}

fn projected_slot_duration_seconds(
    service_config: &Option<
        crate::doctor_timeslot::get_timeslot::repo::DoctorServiceConfigSnapshot,
    >,
    requested_duration_minutes: Option<i32>,
    legacy_slot_duration_seconds: i64,
    require_v2_snapshot: bool,
) -> Option<i64> {
    match service_config {
        Some(config)
            if requested_duration_minutes
                .is_none_or(|duration| duration == config.duration_minutes) =>
        {
            Some(i64::from(config.duration_minutes) * 60)
        }
        Some(_) => None,
        None if require_v2_snapshot => None,
        None if requested_duration_minutes
            .is_none_or(|duration| i64::from(duration) * 60 == legacy_slot_duration_seconds) =>
        {
            Some(legacy_slot_duration_seconds)
        }
        None => None,
    }
}

fn generate_available_timeslots(
    doctor_id: Uuid,
    config: &ScheduleAvailableConfig,
    reserved: &[ReservedWindow],
    from_epoch: i64,
    to_epoch: i64,
    slot_duration_seconds: i64,
    consultation_channel: ConsultationChannel,
) -> Result<Vec<AvailableTimeslot>, GetDoctorTimeslotError> {
    if slot_duration_seconds <= 0 || slot_duration_seconds % 60 != 0 {
        return Err(GetDoctorTimeslotError::InvalidRequest(
            "slot duration must be a positive whole number of minutes".to_string(),
        ));
    }

    let tz = TimeZone::get(&config.timezone).map_err(|e| {
        GetDoctorTimeslotError::InvalidRequest(format!(
            "invalid timezone '{}': {e}",
            config.timezone
        ))
    })?;
    let mut date = Timestamp::from_second(from_epoch)
        .map_err(|e| GetDoctorTimeslotError::InvalidRequest(e.to_string()))?
        .to_zoned(tz.clone())
        .date();
    let end_date = Timestamp::from_second(to_epoch)
        .map_err(|e| GetDoctorTimeslotError::InvalidRequest(e.to_string()))?
        .to_zoned(tz.clone())
        .date();
    let slot_duration_minutes = (slot_duration_seconds / 60) as i32;
    let mut slots = Vec::new();

    loop {
        let periods = periods_for_date(config, date)?;
        for period in periods {
            let mut start_minute = period.start_time;
            while start_minute + slot_duration_minutes <= period.end_time {
                let start_epoch = date_minute_to_epoch(date, start_minute, tz.clone())?;
                let end_epoch = start_epoch + slot_duration_seconds;

                if start_epoch >= from_epoch
                    && end_epoch <= to_epoch
                    && !overlaps_reserved(start_epoch, end_epoch, reserved)
                {
                    slots.push(AvailableTimeslot {
                        timeslot_id: stable_timeslot_id(
                            doctor_id,
                            start_epoch,
                            end_epoch,
                            &consultation_channel,
                        ),
                        start: Timestamp::from_second(start_epoch)
                            .map_err(|e| GetDoctorTimeslotError::InvalidRequest(e.to_string()))?
                            .to_string(),
                        end: Timestamp::from_second(end_epoch)
                            .map_err(|e| GetDoctorTimeslotError::InvalidRequest(e.to_string()))?
                            .to_string(),
                        start_epoch,
                        end_epoch,
                    });
                }

                start_minute += slot_duration_minutes;
            }
        }

        if date >= end_date {
            break;
        }
        date = date
            .tomorrow()
            .map_err(|e| GetDoctorTimeslotError::InvalidRequest(e.to_string()))?;
    }

    Ok(slots)
}

pub(crate) fn timeslot_within_schedule(
    config: &ScheduleAvailableConfig,
    start_epoch: i64,
    end_epoch: i64,
) -> Result<bool, GetDoctorTimeslotError> {
    if start_epoch >= end_epoch {
        return Ok(false);
    }

    let tz = TimeZone::get(&config.timezone).map_err(|e| {
        GetDoctorTimeslotError::InvalidRequest(format!(
            "invalid timezone '{}': {e}",
            config.timezone
        ))
    })?;
    let start = Timestamp::from_second(start_epoch)
        .map_err(|e| GetDoctorTimeslotError::InvalidRequest(e.to_string()))?
        .to_zoned(tz.clone());
    let end = Timestamp::from_second(end_epoch)
        .map_err(|e| GetDoctorTimeslotError::InvalidRequest(e.to_string()))?
        .to_zoned(tz.clone());

    if start.date() != end.date() {
        return Ok(false);
    }

    let start_time = start.time();
    let end_time = end.time();
    let start_minute = i32::from(start_time.hour()) * 60 + i32::from(start_time.minute());
    let end_minute = i32::from(end_time.hour()) * 60 + i32::from(end_time.minute());

    Ok(periods_for_date(config, start.date())?
        .iter()
        .any(|period| start_minute >= period.start_time && end_minute <= period.end_time))
}

fn periods_for_date(
    config: &ScheduleAvailableConfig,
    date: Date,
) -> Result<Vec<TimePeriod>, GetDoctorTimeslotError> {
    let date_string = date.to_string();
    if let Some(specific) = config
        .specific_date
        .iter()
        .find(|specific| specific.date == date_string)
    {
        return Ok(specific.periods.clone());
    }

    let day_of_week = date
        .strftime("%u")
        .to_string()
        .parse::<i32>()
        .map_err(|e| GetDoctorTimeslotError::InvalidRequest(e.to_string()))?;
    Ok(config
        .days_of_week
        .get(&day_of_week)
        .cloned()
        .unwrap_or_default())
}

fn date_minute_to_epoch(
    date: Date,
    minute_of_day: i32,
    tz: TimeZone,
) -> Result<i64, GetDoctorTimeslotError> {
    let hour = minute_of_day / 60;
    let minute = minute_of_day % 60;
    Ok(date
        .at(hour as i8, minute as i8, 0, 0)
        .to_zoned(tz)
        .map_err(|e| GetDoctorTimeslotError::InvalidRequest(e.to_string()))?
        .timestamp()
        .as_second())
}

fn local_date_epoch_range(
    date: &str,
    timezone: &str,
) -> Result<(i64, i64), GetDoctorTimeslotError> {
    let date: Date = date.parse().map_err(|e| {
        GetDoctorTimeslotError::InvalidRequest(format!("invalid date '{date}': {e}"))
    })?;
    let tz = TimeZone::get(timezone).map_err(|e| {
        GetDoctorTimeslotError::InvalidRequest(format!("invalid timezone '{timezone}': {e}"))
    })?;
    let next_date = date
        .tomorrow()
        .map_err(|e| GetDoctorTimeslotError::InvalidRequest(e.to_string()))?;

    Ok((
        date.at(0, 0, 0, 0)
            .to_zoned(tz.clone())
            .map_err(|e| GetDoctorTimeslotError::InvalidRequest(e.to_string()))?
            .timestamp()
            .as_second(),
        next_date
            .at(0, 0, 0, 0)
            .to_zoned(tz)
            .map_err(|e| GetDoctorTimeslotError::InvalidRequest(e.to_string()))?
            .timestamp()
            .as_second(),
    ))
}

fn stable_timeslot_id(
    doctor_id: Uuid,
    start_epoch: i64,
    end_epoch: i64,
    consultation_channel: &ConsultationChannel,
) -> String {
    let channel: String = consultation_channel.clone().into();
    format!("{doctor_id}:{start_epoch}:{end_epoch}:{channel}")
}

fn overlaps_reserved(start_epoch: i64, end_epoch: i64, reserved: &[ReservedWindow]) -> bool {
    reserved
        .iter()
        .any(|window| start_epoch < window.end_epoch && end_epoch > window.start_epoch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consultation_config::model::DateWithTimePeriods;
    use crate::doctor_timeslot::get_timeslot::repo::{
        DoctorScheduleCandidate, DoctorServiceConfigSnapshot, DoctorTimeslotConfigSnapshot,
        DoctorTimeslotIdentity,
    };
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    fn config() -> ScheduleAvailableConfig {
        let mut days_of_week = BTreeMap::new();
        days_of_week.insert(
            4,
            vec![TimePeriod {
                start_time: 9 * 60,
                end_time: 10 * 60,
            }],
        );
        ScheduleAvailableConfig {
            specific_date: vec![DateWithTimePeriods {
                date: "2026-06-18".to_string(),
                periods: vec![TimePeriod {
                    start_time: 13 * 60,
                    end_time: 14 * 60,
                }],
            }],
            timezone: "Asia/Bangkok".to_string(),
            days_of_week,
        }
    }

    struct RecordingRepo {
        identity: Option<DoctorTimeslotIdentity>,
        snapshot: DoctorTimeslotConfigSnapshot,
        reserved_windows: Vec<ReservedWindow>,
        schedule_candidates: Vec<DoctorScheduleCandidate>,
        identity_lookups: Mutex<Vec<(i64, i64)>>,
        identity_by_doctor_id_lookups: Mutex<Vec<uuid::Uuid>>,
        schedule_candidate_lookups: Mutex<usize>,
        config_lookups: Mutex<Vec<uuid::Uuid>>,
        reserved_window_lookups: Mutex<Vec<(i64, i64, i64)>>,
    }

    impl RecordingRepo {
        fn new(identity: Option<DoctorTimeslotIdentity>) -> Self {
            Self {
                identity,
                snapshot: DoctorTimeslotConfigSnapshot {
                    schedule_available: true,
                    schedule_config: config(),
                    service_config: None,
                },
                reserved_windows: Vec::new(),
                schedule_candidates: Vec::new(),
                identity_lookups: Mutex::new(Vec::new()),
                identity_by_doctor_id_lookups: Mutex::new(Vec::new()),
                schedule_candidate_lookups: Mutex::new(0),
                config_lookups: Mutex::new(Vec::new()),
                reserved_window_lookups: Mutex::new(Vec::new()),
            }
        }

        fn with_schedule_available(mut self, schedule_available: bool) -> Self {
            self.snapshot.schedule_available = schedule_available;
            self
        }

        fn with_service_config(mut self, service_config: DoctorServiceConfigSnapshot) -> Self {
            self.snapshot.service_config = Some(service_config);
            self
        }

        fn with_schedule_candidates(
            mut self,
            schedule_candidates: Vec<DoctorScheduleCandidate>,
        ) -> Self {
            self.schedule_candidates = schedule_candidates;
            self
        }

        fn identity_lookups(&self) -> Vec<(i64, i64)> {
            self.identity_lookups
                .lock()
                .expect("mutex poisoned")
                .clone()
        }

        fn config_lookups(&self) -> Vec<uuid::Uuid> {
            self.config_lookups.lock().expect("mutex poisoned").clone()
        }

        fn identity_by_doctor_id_lookups(&self) -> Vec<uuid::Uuid> {
            self.identity_by_doctor_id_lookups
                .lock()
                .expect("mutex poisoned")
                .clone()
        }

        fn schedule_candidate_lookups(&self) -> usize {
            *self
                .schedule_candidate_lookups
                .lock()
                .expect("mutex poisoned")
        }

        fn reserved_window_lookups(&self) -> Vec<(i64, i64, i64)> {
            self.reserved_window_lookups
                .lock()
                .expect("mutex poisoned")
                .clone()
        }
    }

    #[async_trait::async_trait]
    impl GetDoctorTimeslotRepo for RecordingRepo {
        async fn resolve_doctor_identity(
            &self,
            doctor_account_id: i64,
            doctor_profile_id: i64,
        ) -> Result<Option<DoctorTimeslotIdentity>, anyhow::Error> {
            self.identity_lookups
                .lock()
                .expect("mutex poisoned")
                .push((doctor_account_id, doctor_profile_id));
            Ok(self.identity.clone())
        }

        async fn resolve_doctor_identity_by_doctor_id(
            &self,
            doctor_id: uuid::Uuid,
        ) -> Result<Option<DoctorTimeslotIdentity>, anyhow::Error> {
            self.identity_by_doctor_id_lookups
                .lock()
                .expect("mutex poisoned")
                .push(doctor_id);
            Ok(self
                .identity
                .clone()
                .filter(|identity| identity.doctor_id == doctor_id))
        }

        async fn list_schedule_available_doctors(
            &self,
        ) -> Result<Vec<DoctorScheduleCandidate>, anyhow::Error> {
            *self
                .schedule_candidate_lookups
                .lock()
                .expect("mutex poisoned") += 1;
            Ok(self.schedule_candidates.clone())
        }

        async fn get_config_snapshot(
            &self,
            doctor_id: uuid::Uuid,
        ) -> Result<DoctorTimeslotConfigSnapshot, anyhow::Error> {
            self.config_lookups
                .lock()
                .expect("mutex poisoned")
                .push(doctor_id);
            Ok(self.snapshot.clone())
        }

        async fn list_reserved_windows(
            &self,
            doctor_profile_id: i64,
            from_epoch: i64,
            to_epoch: i64,
        ) -> Result<Vec<ReservedWindow>, anyhow::Error> {
            self.reserved_window_lookups
                .lock()
                .expect("mutex poisoned")
                .push((doctor_profile_id, from_epoch, to_epoch));
            Ok(self.reserved_windows.clone())
        }
    }

    fn doctor_identity() -> DoctorTimeslotIdentity {
        DoctorTimeslotIdentity {
            doctor_id: uuid::uuid!("018f1414-5e0e-7c2a-b908-7b1967f2b401"),
            doctor_account_id: 42,
            doctor_profile_id: 84,
            is_active: true,
        }
    }

    fn service_with_repo(repo: Arc<RecordingRepo>) -> GetDoctorTimeslotService {
        GetDoctorTimeslotService::new(repo, 30 * 60)
    }

    #[test]
    fn timeslot_within_schedule_accepts_slot_inside_specific_date_period() {
        assert!(timeslot_within_schedule(&config(), 1_781_762_400, 1_781_764_200).unwrap());
    }

    #[test]
    fn timeslot_within_schedule_rejects_time_outside_period() {
        assert!(!timeslot_within_schedule(&config(), 1_781_769_600, 1_781_771_400).unwrap());
    }

    #[test]
    fn specific_date_overrides_weekly_and_reserved_windows_are_subtracted() {
        let slots = generate_available_timeslots(
            doctor_identity().doctor_id,
            &config(),
            &[ReservedWindow {
                booking_id: "BK1".to_string(),
                start_epoch: 1781764200,
                end_epoch: 1781766000,
            }],
            1781715600,
            1781802000,
            30 * 60,
            ConsultationChannel::Video,
        )
        .expect("test setup should succeed");

        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].start_epoch, 1781762400);
    }

    #[tokio::test]
    async fn service_rejects_invalid_range_before_repo_lookup() {
        let repo = Arc::new(RecordingRepo::new(Some(doctor_identity())));
        let service = service_with_repo(repo.clone());

        let error = service
            .get_available_timeslots(42, 84, "2026-06-18T01:00:00Z", "2026-06-18T01:00:00Z", None)
            .await
            .expect_err("invalid range should fail");

        assert!(matches!(error, GetDoctorTimeslotError::InvalidRequest(_)));
        assert!(repo.identity_lookups().is_empty());
        assert!(repo.config_lookups().is_empty());
        assert!(repo.reserved_window_lookups().is_empty());
    }

    #[tokio::test]
    async fn service_returns_empty_when_schedule_unavailable_without_reserved_lookup() {
        let identity = doctor_identity();
        let repo =
            Arc::new(RecordingRepo::new(Some(identity.clone())).with_schedule_available(false));
        let service = service_with_repo(repo.clone());

        let response = service
            .get_available_timeslots(42, 84, "2026-06-17T17:00:00Z", "2026-06-18T17:00:00Z", None)
            .await
            .expect("schedule-unavailable doctor should return empty timeslots");

        assert_eq!(response.doctor_id, identity.doctor_id);
        assert!(response.timeslots.is_empty());
        assert_eq!(repo.identity_lookups(), vec![(42, 84)]);
        assert_eq!(repo.config_lookups(), vec![identity.doctor_id]);
        assert!(repo.reserved_window_lookups().is_empty());
    }

    #[tokio::test]
    async fn service_returns_not_provisioned_when_identity_missing() {
        let repo = Arc::new(RecordingRepo::new(None));
        let service = service_with_repo(repo.clone());

        let error = service
            .get_available_timeslots(42, 84, "2026-06-17T17:00:00Z", "2026-06-18T17:00:00Z", None)
            .await
            .expect_err("missing identity should fail before config lookup");

        assert!(matches!(
            error,
            GetDoctorTimeslotError::DoctorIdentityNotProvisioned
        ));
        assert_eq!(repo.identity_lookups(), vec![(42, 84)]);
        assert!(repo.config_lookups().is_empty());
        assert!(repo.reserved_window_lookups().is_empty());
    }

    #[tokio::test]
    async fn star_gate_doctor_search_returns_only_doctors_with_scheduled_slots() {
        let doctor = doctor_identity();
        let repo = Arc::new(RecordingRepo::new(None).with_schedule_candidates(vec![
            DoctorScheduleCandidate {
                identity: doctor.clone(),
                schedule_available: true,
                schedule_config: config(),
                service_config: None,
            },
            DoctorScheduleCandidate {
                identity: DoctorTimeslotIdentity {
                    doctor_id: uuid::uuid!("018f1414-5e0e-7c2a-b908-7b1967f2b402"),
                    doctor_account_id: 43,
                    doctor_profile_id: 85,
                    is_active: true,
                },
                schedule_available: false,
                schedule_config: config(),
                service_config: None,
            },
        ]));
        let service = service_with_repo(repo.clone());

        let response = service
            .search_available_doctors(
                "2026-06-18",
                "Asia/Bangkok",
                Some(ConsultationChannel::Video),
            )
            .await
            .expect("doctor search should succeed");

        assert_eq!(response.date, "2026-06-18");
        assert_eq!(response.timezone, "Asia/Bangkok");
        assert_eq!(response.doctors.len(), 1);
        assert_eq!(response.doctors[0].doctor_id, doctor.doctor_id);
        assert_eq!(
            response.doctors[0].doctor_account_id,
            doctor.doctor_account_id
        );
        assert_eq!(
            response.doctors[0].doctor_profile_id,
            doctor.doctor_profile_id
        );
        assert_eq!(response.doctors[0].available_timeslot_count, 2);
        assert_eq!(
            response.doctors[0].next_available_timeslot_id.as_deref(),
            Some("018f1414-5e0e-7c2a-b908-7b1967f2b401:1781762400:1781764200:video")
        );
        assert_eq!(repo.schedule_candidate_lookups(), 1);
    }

    #[tokio::test]
    async fn star_gate_doctor_search_returns_empty_when_no_scheduled_availability_matches() {
        let repo = Arc::new(RecordingRepo::new(None).with_schedule_candidates(vec![
            DoctorScheduleCandidate {
                identity: doctor_identity(),
                schedule_available: true,
                schedule_config: ScheduleAvailableConfig::default(),
                service_config: None,
            },
        ]));
        let service = service_with_repo(repo);

        let response = service
            .search_available_doctors("2026-06-18", "Asia/Bangkok", None)
            .await
            .expect("doctor search should succeed");

        assert!(response.doctors.is_empty());
    }

    #[tokio::test]
    async fn star_gate_scheduled_availability_by_doctor_date_returns_stable_timeslot_ids() {
        let doctor = doctor_identity();
        let repo = Arc::new(RecordingRepo::new(Some(doctor.clone())));
        let service = service_with_repo(repo.clone());

        let response = service
            .get_scheduled_availability_by_doctor_date(
                doctor.doctor_id,
                "2026-06-18",
                "Asia/Bangkok",
                Some(ConsultationChannel::Video),
            )
            .await
            .expect("scheduled availability should succeed");

        assert_eq!(response.doctor_id, doctor.doctor_id);
        assert_eq!(response.timeslots.len(), 2);
        assert_eq!(
            response.timeslots[0].timeslot_id,
            "018f1414-5e0e-7c2a-b908-7b1967f2b401:1781762400:1781764200:video"
        );
        assert_eq!(response.timeslots[0].start_epoch, 1781762400);
        assert_eq!(response.timeslots[0].end_epoch, 1781764200);
        assert_eq!(repo.identity_by_doctor_id_lookups(), vec![doctor.doctor_id]);
        assert_eq!(repo.config_lookups(), vec![doctor.doctor_id]);
        assert_eq!(
            repo.reserved_window_lookups(),
            vec![(doctor.doctor_profile_id, 1781715600, 1781802000)]
        );
    }

    #[tokio::test]
    async fn doctor_search_filters_projected_channels_and_uses_projected_duration() {
        let doctor = doctor_identity();
        let repo = Arc::new(
            RecordingRepo::new(Some(doctor.clone()))
                .with_service_config(DoctorServiceConfigSnapshot {
                    channels: vec!["video".to_string()],
                    duration_minutes: 25,
                })
                .with_schedule_candidates(vec![DoctorScheduleCandidate {
                    identity: doctor.clone(),
                    schedule_available: true,
                    schedule_config: config(),
                    service_config: Some(DoctorServiceConfigSnapshot {
                        channels: vec!["video".to_string()],
                        duration_minutes: 25,
                    }),
                }]),
        );
        let service = service_with_repo(repo);

        let chat = service
            .search_available_doctors(
                "2026-06-18",
                "Asia/Bangkok",
                Some(ConsultationChannel::Chat),
            )
            .await
            .unwrap();
        assert!(chat.doctors.is_empty());

        let response = service
            .get_scheduled_availability_by_doctor_date(
                doctor.doctor_id,
                "2026-06-18",
                "Asia/Bangkok",
                Some(ConsultationChannel::Video),
            )
            .await
            .unwrap();
        assert_eq!(
            response.timeslots[0].end_epoch - response.timeslots[0].start_epoch,
            25 * 60
        );
    }

    #[tokio::test]
    async fn require_v2_snapshot_hides_legacy_availability() {
        let repo = Arc::new(RecordingRepo::new(Some(doctor_identity())));
        let service = GetDoctorTimeslotService::new_with_v2_snapshot(repo, 30 * 60, true);
        let response = service
            .get_available_timeslots(
                42,
                84,
                "2026-06-17T17:00:00Z",
                "2026-06-18T17:00:00Z",
                Some(ConsultationChannel::Video),
            )
            .await
            .unwrap();
        assert!(response.timeslots.is_empty());
    }
}
