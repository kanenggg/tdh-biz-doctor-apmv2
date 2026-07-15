use std::collections::BTreeMap;

use jiff::civil::Date;

use super::model::{ScheduleAvailableConfig, TimePeriod};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleConfigValidationError {
    Invalid(String),
    ConflictTimeOverlap { days: Vec<i32> },
}

pub fn validate_schedule_config(
    config: &ScheduleAvailableConfig,
) -> Result<(), ScheduleConfigValidationError> {
    if jiff::tz::TimeZone::get(&config.timezone).is_err() {
        return Err(ScheduleConfigValidationError::Invalid(
            "timezone must be a valid IANA timezone name".to_string(),
        ));
    }

    for (day, periods) in &config.days_of_week {
        if !(1..=7).contains(day) {
            return Err(ScheduleConfigValidationError::Invalid(
                "daysOfWeek keys must be between 1 and 7".to_string(),
            ));
        }

        validate_periods(periods)?;
    }

    for date_config in &config.specific_date {
        validate_specific_date(&date_config.date)?;
        validate_periods(&date_config.periods)?;
    }

    let days = conflict_time_overlap_days(&config.days_of_week);
    if !days.is_empty() {
        return Err(ScheduleConfigValidationError::ConflictTimeOverlap { days });
    }

    Ok(())
}

fn conflict_time_overlap_days(day_of_week: &BTreeMap<i32, Vec<TimePeriod>>) -> Vec<i32> {
    day_of_week
        .iter()
        .filter_map(|(day, periods)| has_time_overlap(periods).then_some(*day))
        .collect()
}

fn validate_periods(periods: &[TimePeriod]) -> Result<(), ScheduleConfigValidationError> {
    for period in periods {
        if period.start_time < 0 || period.end_time > 1440 || period.start_time >= period.end_time {
            return Err(ScheduleConfigValidationError::Invalid(
                "periods must satisfy 0 <= startTime < endTime <= 1440".to_string(),
            ));
        }
    }

    Ok(())
}

fn validate_specific_date(date: &str) -> Result<(), ScheduleConfigValidationError> {
    let bytes = date.as_bytes();
    let has_yyyy_mm_dd_shape = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[8..].iter().all(u8::is_ascii_digit);

    if !has_yyyy_mm_dd_shape || Date::strptime("%Y-%m-%d", date).is_err() {
        return Err(ScheduleConfigValidationError::Invalid(
            "specificDate.date must use yyyy-mm-dd format".to_string(),
        ));
    }

    Ok(())
}

fn has_time_overlap(periods: &[TimePeriod]) -> bool {
    let mut periods = periods.iter().collect::<Vec<_>>();
    periods.sort_by_key(|period| (period.start_time, period.end_time));

    periods
        .windows(2)
        .any(|window| window[1].start_time < window[0].end_time)
}
