use std::{sync::Arc, time::Duration};

use crate::event_outbox::service::EventOutboxDispatchService;

#[derive(Debug, Clone)]
pub(crate) struct EventOutboxWorkerConfig {
    pub enabled: bool,
    pub poll_interval_seconds: u64,
    pub batch_size: i64,
}

pub(crate) fn spawn_event_outbox_worker(
    service: Arc<EventOutboxDispatchService>,
    config: EventOutboxWorkerConfig,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if !config.enabled {
            tracing::info!("Event outbox worker disabled");
            return;
        }

        let interval = Duration::from_secs(config.poll_interval_seconds.max(1));
        tracing::info!(
            poll_interval_seconds = interval.as_secs(),
            batch_size = config.batch_size,
            "Event outbox worker started"
        );

        loop {
            match service.dispatch_pending(config.batch_size.max(1)).await {
                Ok(count) if count > 0 => {
                    tracing::info!(published_count = count, "Published outbox events");
                }
                Ok(_) => {}
                Err(error) => tracing::error!(%error, "Event outbox worker failed"),
            }

            tokio::time::sleep(interval).await;
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::RawEventPublisher;
    use crate::event_outbox::repo::{EventOutboxRepo, OutboxEventRow};
    use std::sync::Mutex;
    use tokio::sync::Notify;
    use uuid::Uuid;

    #[derive(Default)]
    struct RecordingRepo {
        claim_calls: Mutex<Vec<(i64, i32)>>,
        claimed: Notify,
    }

    #[async_trait::async_trait]
    impl EventOutboxRepo for RecordingRepo {
        async fn claim_pending_events(
            &self,
            batch_size: i64,
            lock_seconds: i32,
        ) -> Result<Vec<OutboxEventRow>, anyhow::Error> {
            self.claim_calls
                .lock()
                .expect("claim calls mutex should not be poisoned")
                .push((batch_size, lock_seconds));
            self.claimed.notify_waiters();
            Ok(vec![])
        }

        async fn mark_published(&self, _event_id: Uuid) -> Result<(), anyhow::Error> {
            panic!("worker tests should not mark events published")
        }

        async fn mark_failed(&self, _event_id: Uuid, _error: &str) -> Result<(), anyhow::Error> {
            panic!("worker tests should not mark events failed")
        }
    }

    struct PanickingPublisher;

    #[async_trait::async_trait]
    impl RawEventPublisher for PanickingPublisher {
        async fn publish_raw(
            &self,
            _topic_name: &str,
            _event_type: &str,
            _event_id: Uuid,
            _payload: Vec<u8>,
        ) -> Result<(), anyhow::Error> {
            panic!("worker tests should not publish events")
        }
    }

    fn dispatch_service(repo: Arc<RecordingRepo>) -> Arc<EventOutboxDispatchService> {
        Arc::new(EventOutboxDispatchService::new(
            repo,
            Arc::new(PanickingPublisher),
            30,
        ))
    }

    #[tokio::test]
    async fn disabled_worker_exits_without_dispatching() {
        let repo = Arc::new(RecordingRepo::default());
        let service = dispatch_service(repo.clone());

        let handle = spawn_event_outbox_worker(
            service,
            EventOutboxWorkerConfig {
                enabled: false,
                poll_interval_seconds: 0,
                batch_size: 0,
            },
        );

        tokio::time::timeout(Duration::from_millis(100), handle)
            .await
            .expect("disabled worker should exit promptly")
            .expect("disabled worker should not panic");

        assert!(
            repo.claim_calls
                .lock()
                .expect("claim calls mutex should not be poisoned")
                .is_empty(),
            "disabled worker must not dispatch outbox events"
        );
    }

    #[tokio::test]
    async fn enabled_worker_clamps_zero_batch_size_and_poll_interval() {
        let repo = Arc::new(RecordingRepo::default());
        let first_claim = repo.claimed.notified();
        let service = dispatch_service(repo.clone());

        let handle = spawn_event_outbox_worker(
            service,
            EventOutboxWorkerConfig {
                enabled: true,
                poll_interval_seconds: 0,
                batch_size: 0,
            },
        );

        tokio::time::timeout(Duration::from_millis(100), first_claim)
            .await
            .expect("enabled worker should dispatch once promptly");

        tokio::time::sleep(Duration::from_millis(100)).await;
        handle.abort();
        let _ = handle.await;

        assert_eq!(
            repo.claim_calls
                .lock()
                .expect("claim calls mutex should not be poisoned")
                .as_slice(),
            &[(1, 30)],
            "worker should clamp batch_size to 1 and sleep at least one second before the next dispatch"
        );
    }
}
