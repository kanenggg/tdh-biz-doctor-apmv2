use std::sync::Arc;

use crate::event::RawEventPublisher;
use crate::event_outbox::repo::EventOutboxRepo;

#[derive(Debug, thiserror::Error)]
pub(crate) enum EventOutboxDispatchError {
    #[error("repository error: {0}")]
    Repository(#[from] anyhow::Error),
}

pub(crate) struct EventOutboxDispatchService {
    repo: Arc<dyn EventOutboxRepo>,
    publisher: Arc<dyn RawEventPublisher>,
    lock_seconds: i32,
}

impl EventOutboxDispatchService {
    pub(crate) fn new(
        repo: Arc<dyn EventOutboxRepo>,
        publisher: Arc<dyn RawEventPublisher>,
        lock_seconds: i32,
    ) -> Self {
        Self {
            repo,
            publisher,
            lock_seconds,
        }
    }

    pub(crate) async fn dispatch_pending(
        &self,
        batch_size: i64,
    ) -> Result<usize, EventOutboxDispatchError> {
        let events = self
            .repo
            .claim_pending_events(batch_size, self.lock_seconds)
            .await?;
        let mut published_count = 0;

        for event in events {
            let payload = match serde_json::to_vec(&event.payload) {
                Ok(payload) => payload,
                Err(error) => {
                    self.repo
                        .mark_failed(
                            event.event_id,
                            &format!("payload serialization failed: {error}"),
                        )
                        .await?;
                    continue;
                }
            };

            match self
                .publisher
                .publish_raw(&event.topic, &event.event_type, event.event_id, payload)
                .await
            {
                Ok(()) => {
                    self.repo.mark_published(event.event_id).await?;
                    published_count += 1;
                }
                Err(error) => {
                    self.repo
                        .mark_failed(event.event_id, &error.to_string())
                        .await?;
                }
            }
        }

        Ok(published_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_outbox::repo::{EventOutboxRepo, OutboxEventRow};
    use std::sync::Mutex;
    use uuid::Uuid;

    struct FakeRepo {
        events: Mutex<Vec<OutboxEventRow>>,
        claim_calls: Mutex<Vec<(i64, i32)>>,
        published: Mutex<Vec<Uuid>>,
        failed: Mutex<Vec<(Uuid, String)>>,
    }

    impl FakeRepo {
        fn new(events: Vec<OutboxEventRow>) -> Self {
            Self {
                events: Mutex::new(events),
                claim_calls: Mutex::new(vec![]),
                published: Mutex::new(vec![]),
                failed: Mutex::new(vec![]),
            }
        }
    }

    #[async_trait::async_trait]
    impl EventOutboxRepo for FakeRepo {
        async fn claim_pending_events(
            &self,
            batch_size: i64,
            lock_seconds: i32,
        ) -> Result<Vec<OutboxEventRow>, anyhow::Error> {
            self.claim_calls
                .lock()
                .expect("claim calls mutex should not be poisoned")
                .push((batch_size, lock_seconds));
            Ok(self
                .events
                .lock()
                .expect("events mutex should not be poisoned")
                .clone())
        }

        async fn mark_published(&self, event_id: Uuid) -> Result<(), anyhow::Error> {
            self.published
                .lock()
                .expect("published mutex should not be poisoned")
                .push(event_id);
            Ok(())
        }

        async fn mark_failed(&self, event_id: Uuid, error: &str) -> Result<(), anyhow::Error> {
            self.failed
                .lock()
                .expect("failed mutex should not be poisoned")
                .push((event_id, error.to_string()));
            Ok(())
        }
    }

    struct FakePublisher {
        error: Option<String>,
        published: Mutex<Vec<(String, String, Uuid, Vec<u8>)>>,
    }

    impl FakePublisher {
        fn succeeding() -> Self {
            Self {
                error: None,
                published: Mutex::new(vec![]),
            }
        }

        fn failing(error: &str) -> Self {
            Self {
                error: Some(error.to_string()),
                published: Mutex::new(vec![]),
            }
        }
    }

    #[async_trait::async_trait]
    impl RawEventPublisher for FakePublisher {
        async fn publish_raw(
            &self,
            topic_name: &str,
            event_type: &str,
            event_id: Uuid,
            payload: Vec<u8>,
        ) -> Result<(), anyhow::Error> {
            self.published
                .lock()
                .expect("published mutex should not be poisoned")
                .push((
                    topic_name.to_string(),
                    event_type.to_string(),
                    event_id,
                    payload,
                ));

            match &self.error {
                Some(error) => Err(anyhow::anyhow!(error.clone())),
                None => Ok(()),
            }
        }
    }

    fn outbox_event(event_id: Uuid) -> OutboxEventRow {
        OutboxEventRow {
            event_id,
            topic: "consultation-events".to_string(),
            event_type: "ConsultationBooked".to_string(),
            payload: serde_json::json!({
                "bookingId": "booking-1",
                "bookedAt": 1_700_000_000
            }),
        }
    }

    #[tokio::test]
    async fn dispatch_pending_marks_published_and_counts_successful_publish() {
        let event_id = Uuid::new_v4();
        let event = outbox_event(event_id);
        let expected_payload = serde_json::to_vec(&event.payload).unwrap();
        let repo = Arc::new(FakeRepo::new(vec![event]));
        let publisher = Arc::new(FakePublisher::succeeding());
        let service = EventOutboxDispatchService::new(repo.clone(), publisher.clone(), 60);

        let published_count = service.dispatch_pending(25).await.unwrap();

        assert_eq!(published_count, 1);
        assert_eq!(
            repo.claim_calls
                .lock()
                .expect("claim calls mutex should not be poisoned")
                .as_slice(),
            &[(25, 60)]
        );
        assert_eq!(
            repo.published
                .lock()
                .expect("published mutex should not be poisoned")
                .as_slice(),
            &[event_id]
        );
        assert!(
            repo.failed
                .lock()
                .expect("failed mutex should not be poisoned")
                .is_empty()
        );
        assert_eq!(
            publisher
                .published
                .lock()
                .expect("published mutex should not be poisoned")
                .as_slice(),
            &[(
                "consultation-events".to_string(),
                "ConsultationBooked".to_string(),
                event_id,
                expected_payload,
            )]
        );
    }

    #[tokio::test]
    async fn dispatch_pending_marks_failed_and_does_not_count_publish_error() {
        let event_id = Uuid::new_v4();
        let repo = Arc::new(FakeRepo::new(vec![outbox_event(event_id)]));
        let publisher = Arc::new(FakePublisher::failing("pubsub unavailable"));
        let service = EventOutboxDispatchService::new(repo.clone(), publisher.clone(), 60);

        let published_count = service.dispatch_pending(10).await.unwrap();

        assert_eq!(published_count, 0);
        assert!(
            repo.published
                .lock()
                .expect("published mutex should not be poisoned")
                .is_empty()
        );
        assert_eq!(
            repo.failed
                .lock()
                .expect("failed mutex should not be poisoned")
                .as_slice(),
            &[(event_id, "pubsub unavailable".to_string())]
        );
        assert_eq!(
            publisher
                .published
                .lock()
                .expect("published mutex should not be poisoned")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn retry_reuses_the_persisted_event_id_and_payload() {
        let event_id = Uuid::from_u128(1);
        let event = outbox_event(event_id);
        let expected_payload = serde_json::to_vec(&event.payload).unwrap();

        let failing_repo = Arc::new(FakeRepo::new(vec![event.clone()]));
        let failing_publisher = Arc::new(FakePublisher::failing("temporary failure"));
        EventOutboxDispatchService::new(failing_repo.clone(), failing_publisher.clone(), 60)
            .dispatch_pending(1)
            .await
            .unwrap();

        let retry_repo = Arc::new(FakeRepo::new(vec![event]));
        let retry_publisher = Arc::new(FakePublisher::succeeding());
        EventOutboxDispatchService::new(retry_repo, retry_publisher.clone(), 60)
            .dispatch_pending(1)
            .await
            .unwrap();

        assert_eq!(
            failing_publisher
                .published
                .lock()
                .expect("published mutex should not be poisoned")
                .as_slice(),
            &[(
                "consultation-events".to_string(),
                "ConsultationBooked".to_string(),
                event_id,
                expected_payload.clone(),
            )]
        );
        assert_eq!(
            retry_publisher
                .published
                .lock()
                .expect("published mutex should not be poisoned")
                .as_slice(),
            &[(
                "consultation-events".to_string(),
                "ConsultationBooked".to_string(),
                event_id,
                expected_payload,
            )]
        );
    }
}
