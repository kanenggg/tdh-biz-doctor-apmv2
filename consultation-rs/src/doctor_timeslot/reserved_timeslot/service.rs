use std::sync::Arc;

use super::model::ReservedTimeslotsResponse;
use super::repo::ReservedTimeslotsRepo;

#[derive(Debug, thiserror::Error)]
pub enum ReservedTimeslotsError {
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("repository error: {0}")]
    Repository(#[from] anyhow::Error),
}

/// Resolve a civil `YYYY-MM-DD` (Asia/Bangkok) to `[start, end)` epoch seconds.
/// Retained as a test helper for building known day windows; the request path
/// now receives an explicit `from_datetime`/`to_datetime` range instead.
pub fn bkk_day_range(date: &str) -> Result<(i64, i64), anyhow::Error> {
    let day: jiff::civil::Date = date
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid date '{date}': {e}"))?;
    let tz = jiff::tz::TimeZone::get("Asia/Bangkok")?;
    let start = day
        .at(0, 0, 0, 0)
        .to_zoned(tz.clone())?
        .timestamp()
        .as_second();
    let end = day
        .tomorrow()?
        .at(0, 0, 0, 0)
        .to_zoned(tz)?
        .timestamp()
        .as_second();
    Ok((start, end))
}

/// Parse an RFC-3339 datetime (e.g. `2026-06-17T17:00:00Z`) to epoch seconds.
fn parse_epoch_second(dt: &str) -> Result<i64, ReservedTimeslotsError> {
    let ts: jiff::Timestamp = dt.parse().map_err(|e| {
        ReservedTimeslotsError::InvalidRequest(format!("invalid datetime '{dt}': {e}"))
    })?;
    Ok(ts.as_second())
}

#[derive(Clone)]
pub struct ReservedTimeslotsService {
    repo: Arc<dyn ReservedTimeslotsRepo>,
}

impl ReservedTimeslotsService {
    pub fn new(repo: Arc<dyn ReservedTimeslotsRepo>) -> Self {
        Self { repo }
    }

    pub async fn get_reserved_timeslots(
        &self,
        doctor_profile_id: i32,
        from_datetime: &str,
        to_datetime: &str,
    ) -> Result<ReservedTimeslotsResponse, ReservedTimeslotsError> {
        let from = parse_epoch_second(from_datetime)?;
        let to = parse_epoch_second(to_datetime)?;
        if from >= to {
            return Err(ReservedTimeslotsError::InvalidRequest(
                "fromDatetime must be before toDatetime".to_string(),
            ));
        }

        let reserved_timeslots = self
            .repo
            .find_reserved_timeslots_by_doctor_profile(doctor_profile_id, from, to)
            .await?;
        Ok(ReservedTimeslotsResponse { reserved_timeslots })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor_timeslot::reserved_timeslot::model::ReserveTimeSlot;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct RecordingRepo {
        calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl ReservedTimeslotsRepo for RecordingRepo {
        async fn find_reserved_timeslots_by_doctor_profile(
            &self,
            _doctor_profile_id: i32,
            _day_start: i64,
            _day_end: i64,
        ) -> Result<Vec<ReserveTimeSlot>, anyhow::Error> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        }
    }

    #[test]
    fn bkk_day_range_is_24h_starting_at_bkk_midnight() {
        // 2026-06-18 00:00 +07:00 == 2026-06-17 17:00 UTC == 1781715600
        let (start, end) = bkk_day_range("2026-06-18").unwrap();
        assert_eq!(end - start, 24 * 3600);
        assert_eq!(start, 1781715600);
    }

    #[test]
    fn parses_rfc3339_utc_datetime_to_epoch_second() {
        // 2026-06-17T17:00:00Z == 2026-06-18 00:00 +07:00 == 1781715600
        assert_eq!(
            parse_epoch_second("2026-06-17T17:00:00Z").unwrap(),
            1781715600
        );
    }

    #[tokio::test]
    async fn service_rejects_invalid_range_before_repo_lookup() {
        let repo = Arc::new(RecordingRepo::default());
        let service = ReservedTimeslotsService::new(repo.clone());

        for (from_datetime, to_datetime) in [
            ("2026-06-18T01:00:00Z", "2026-06-18T01:00:00Z"),
            ("2026-06-18T02:00:00Z", "2026-06-18T01:00:00Z"),
        ] {
            let error = service
                .get_reserved_timeslots(84, from_datetime, to_datetime)
                .await
                .expect_err("invalid range should fail");

            assert!(
                error
                    .to_string()
                    .contains("fromDatetime must be before toDatetime")
            );
        }

        assert_eq!(repo.calls.load(Ordering::SeqCst), 0);
    }
}
