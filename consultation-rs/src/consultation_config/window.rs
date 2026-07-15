use jiff::{ToSpan, civil::Date};

use super::model::ScheduleAvailableConfig;

const FORWARD_WINDOW_DAYS: i64 = 90;

pub fn drop_past_specific_dates(config: &mut ScheduleAvailableConfig, today: Date) {
    config
        .specific_date
        .retain(|entry| entry.date.parse::<Date>().is_ok_and(|date| date >= today));
}

pub fn retain_forward_window(config: &mut ScheduleAvailableConfig, today: Date) {
    let end = today
        .checked_add(FORWARD_WINDOW_DAYS.days())
        .unwrap_or(Date::MAX);

    config.specific_date.retain(|entry| {
        entry
            .date
            .parse::<Date>()
            .is_ok_and(|date| date >= today && date <= end)
    });
}
