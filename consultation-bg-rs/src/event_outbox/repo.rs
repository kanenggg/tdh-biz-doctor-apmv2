use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OutboxEventRow {
    pub event_id: Uuid,
    pub topic: String,
    pub event_type: String,
    pub payload: serde_json::Value,
}

const CLAIM_PENDING_EVENTS_SQL: &str = r#"
WITH due AS (
    SELECT event_id
    FROM v2.event_outbox
    WHERE publication_status IN ('PENDING', 'PROCESSING')
      AND (locked_until IS NULL OR locked_until <= NOW())
    ORDER BY created_at ASC
    LIMIT $1
    FOR UPDATE SKIP LOCKED
), claimed AS (
    UPDATE v2.event_outbox e
    SET publication_status = 'PROCESSING',
        locked_until = NOW() + ($2::integer * INTERVAL '1 second'),
        modified_at = NOW()
    FROM due
    WHERE e.event_id = due.event_id
    RETURNING e.event_id, e.topic, e.event_type, e.payload
)
SELECT * FROM claimed
"#;

const MARK_PUBLISHED_SQL: &str = r#"
UPDATE v2.event_outbox
SET publication_status = 'PUBLISHED',
    published_at = NOW(),
    locked_until = NULL,
    last_error = NULL,
    modified_at = NOW()
WHERE event_id = $1
"#;

const MARK_FAILED_SQL: &str = r#"
UPDATE v2.event_outbox
SET publication_status = 'PENDING',
    retry_count = retry_count + 1,
    locked_until = NULL,
    last_error = $2,
    modified_at = NOW()
WHERE event_id = $1
"#;

#[async_trait::async_trait]
pub(crate) trait EventOutboxRepo: Send + Sync {
    async fn claim_pending_events(
        &self,
        batch_size: i64,
        lock_seconds: i32,
    ) -> Result<Vec<OutboxEventRow>, anyhow::Error>;

    async fn mark_published(&self, event_id: Uuid) -> Result<(), anyhow::Error>;

    async fn mark_failed(&self, event_id: Uuid, error: &str) -> Result<(), anyhow::Error>;
}

pub(crate) struct EventOutboxPsql {
    pool: PgPool,
}

impl EventOutboxPsql {
    pub(crate) fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl EventOutboxRepo for EventOutboxPsql {
    async fn claim_pending_events(
        &self,
        batch_size: i64,
        lock_seconds: i32,
    ) -> Result<Vec<OutboxEventRow>, anyhow::Error> {
        let rows = sqlx::query_as::<_, (Uuid, String, String, serde_json::Value)>(
            CLAIM_PENDING_EVENTS_SQL,
        )
        .bind(batch_size)
        .bind(lock_seconds)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to claim outbox events: {e}"))?;

        Ok(rows
            .into_iter()
            .map(|(event_id, topic, event_type, payload)| OutboxEventRow {
                event_id,
                topic,
                event_type,
                payload,
            })
            .collect())
    }

    async fn mark_published(&self, event_id: Uuid) -> Result<(), anyhow::Error> {
        sqlx::query(MARK_PUBLISHED_SQL)
            .bind(event_id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to mark outbox event published: {e}"))?;
        Ok(())
    }

    async fn mark_failed(&self, event_id: Uuid, error: &str) -> Result<(), anyhow::Error> {
        sqlx::query(MARK_FAILED_SQL)
            .bind(event_id)
            .bind(error)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to mark outbox event failed: {e}"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalize_sql(sql: &str) -> String {
        sql.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    fn assert_sql_contains(sql: &str, expected: &str) {
        assert!(
            sql.contains(expected),
            "expected SQL to contain `{expected}`, got `{sql}`"
        );
    }

    #[test]
    fn claim_query_uses_skip_locked_and_only_claims_unlocked_rows() {
        let sql = normalize_sql(CLAIM_PENDING_EVENTS_SQL);

        assert_sql_contains(&sql, "publication_status IN ('PENDING', 'PROCESSING')");
        assert_sql_contains(&sql, "AND (locked_until IS NULL OR locked_until <= NOW())");
        assert_sql_contains(&sql, "ORDER BY created_at ASC");
        assert_sql_contains(&sql, "LIMIT $1");
        assert_sql_contains(&sql, "FOR UPDATE SKIP LOCKED");
        assert_sql_contains(&sql, "UPDATE v2.event_outbox e");
        assert_sql_contains(&sql, "publication_status = 'PROCESSING'");
        assert_sql_contains(
            &sql,
            "locked_until = NOW() + ($2::integer * INTERVAL '1 second')",
        );
        assert_sql_contains(
            &sql,
            "RETURNING e.event_id, e.topic, e.event_type, e.payload",
        );
    }

    #[test]
    fn mark_published_query_clears_lock() {
        let sql = normalize_sql(MARK_PUBLISHED_SQL);

        assert_sql_contains(&sql, "UPDATE v2.event_outbox");
        assert_sql_contains(&sql, "publication_status = 'PUBLISHED'");
        assert_sql_contains(&sql, "published_at = NOW()");
        assert_sql_contains(&sql, "locked_until = NULL");
        assert_sql_contains(&sql, "last_error = NULL");
        assert_sql_contains(&sql, "WHERE event_id = $1");
    }

    #[test]
    fn mark_failed_query_clears_lock_and_increments_retry_count() {
        let sql = normalize_sql(MARK_FAILED_SQL);

        assert_sql_contains(&sql, "UPDATE v2.event_outbox");
        assert_sql_contains(&sql, "publication_status = 'PENDING'");
        assert_sql_contains(&sql, "retry_count = retry_count + 1");
        assert_sql_contains(&sql, "locked_until = NULL");
        assert_sql_contains(&sql, "last_error = $2");
        assert_sql_contains(&sql, "WHERE event_id = $1");
    }
}
