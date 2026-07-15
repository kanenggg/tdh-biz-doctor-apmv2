use sqlx::PgPool;

use super::model::ReserveTimeSlot;

const FIND_RESERVED_TIMESLOTS_SQL: &str = r#"
    SELECT COALESCE(h.booking_id, a.booking_id),
        EXTRACT(EPOCH FROM o.starts_at)::bigint,
        EXTRACT(EPOCH FROM o.ends_at)::bigint
    FROM v2.doctor_occupancy o
    LEFT JOIN v2.appointment_hold h ON h.hold_id = o.hold_id
    LEFT JOIN v2.appointment a ON a.appointment_id = o.appointment_id
    WHERE o.doctor_profile_id = $1
      AND o.occupancy_status = 'ACTIVE'::v2.doctor_occupancy_status_enum
      AND o.starts_at < to_timestamp($3)
      AND o.ends_at > to_timestamp($2)
    ORDER BY o.starts_at
"#;

#[async_trait::async_trait]
pub trait ReservedTimeslotsRepo: Send + Sync {
    /// Active reservations for `doctor_profile_id` whose appointment range
    /// overlaps `[day_start, day_end)` (both epoch seconds, UTC).
    async fn find_reserved_timeslots_by_doctor_profile(
        &self,
        doctor_profile_id: i32,
        day_start: i64,
        day_end: i64,
    ) -> Result<Vec<ReserveTimeSlot>, anyhow::Error>;
}

pub struct ReservedTimeslotsRepoPsql {
    pool: PgPool,
}

impl ReservedTimeslotsRepoPsql {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ReservedTimeslotsRepo for ReservedTimeslotsRepoPsql {
    async fn find_reserved_timeslots_by_doctor_profile(
        &self,
        doctor_profile_id: i32,
        day_start: i64,
        day_end: i64,
    ) -> Result<Vec<ReserveTimeSlot>, anyhow::Error> {
        let rows = sqlx::query_as::<_, (String, i64, i64)>(FIND_RESERVED_TIMESLOTS_SQL)
            .bind(doctor_profile_id)
            .bind(day_start)
            .bind(day_end)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to query reserved timeslots: {}", e))?;

        Ok(rows
            .into_iter()
            .map(|(booking_id, start_time, end_time)| ReserveTimeSlot {
                booking_id,
                start_time,
                end_time,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::FIND_RESERVED_TIMESLOTS_SQL;

    #[test]
    fn reserved_timeslots_query_reads_canonical_active_occupancy() {
        assert!(FIND_RESERVED_TIMESLOTS_SQL.contains("FROM v2.doctor_occupancy"));
        assert!(FIND_RESERVED_TIMESLOTS_SQL.contains("occupancy_status = 'ACTIVE'"));
        assert!(!FIND_RESERVED_TIMESLOTS_SQL.contains("FROM v2.reservation"));
    }
}
