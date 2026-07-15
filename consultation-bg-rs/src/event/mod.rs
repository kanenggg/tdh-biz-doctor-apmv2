use google_cloud_googleapis::pubsub::v1::PubsubMessage;
use google_cloud_pubsub::client::{Client, ClientConfig};
use uuid::Uuid;

const EVENT_TYPE_ATTRIBUTE: &str = "event_type";
const EVENT_ID_ATTRIBUTE: &str = "event_id";

#[async_trait::async_trait]
pub trait RawEventPublisher: Send + Sync {
    async fn publish_raw(
        &self,
        topic_name: &str,
        event_type: &str,
        event_id: Uuid,
        payload: Vec<u8>,
    ) -> Result<(), anyhow::Error>;
}

#[derive(Clone)]
pub struct PubSubConsultationEventPublisher {
    client: Client,
}

impl PubSubConsultationEventPublisher {
    pub async fn new(project_id: &str, emulator_host: Option<&str>) -> Result<Self, anyhow::Error> {
        if let Some(host) = emulator_host {
            if !host.is_empty() {
                // SAFETY: Called once during startup before request handlers are spawned.
                unsafe {
                    std::env::set_var("PUBSUB_EMULATOR_HOST", host);
                }
            }
        }

        let mut config = ClientConfig::default().with_auth().await?;
        config.project_id = Some(pubsub_project_id(project_id, config.project_id.take())?);

        let client = Client::new(config).await?;
        Ok(Self { client })
    }
}

fn pubsub_project_id(
    configured_project_id: &str,
    discovered_project_id: Option<String>,
) -> Result<String, anyhow::Error> {
    let project_id = configured_project_id.trim();
    if project_id.is_empty() {
        if let Some(project_id) = discovered_project_id {
            let project_id = project_id.trim();
            if !project_id.is_empty() {
                return Ok(project_id.to_string());
            }
        }

        anyhow::bail!("google_cloud.project_id is required for Pub/Sub publisher");
    }

    Ok(project_id.to_string())
}

#[async_trait::async_trait]
impl RawEventPublisher for PubSubConsultationEventPublisher {
    async fn publish_raw(
        &self,
        topic_name: &str,
        event_type: &str,
        event_id: Uuid,
        payload: Vec<u8>,
    ) -> Result<(), anyhow::Error> {
        let topic = self.client.topic(topic_name);
        let publisher = topic.new_publisher(None);
        let msg = PubsubMessage {
            data: payload,
            attributes: [
                (EVENT_TYPE_ATTRIBUTE.to_string(), event_type.to_string()),
                (EVENT_ID_ATTRIBUTE.to_string(), event_id.to_string()),
            ]
            .into(),
            ..Default::default()
        };
        let awaiter = publisher.publish(msg).await;
        awaiter
            .get()
            .await
            .map_err(|e| anyhow::anyhow!("PubSub publish failed: {}", e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pubsub_project_id_uses_configured_value() {
        assert_eq!(
            pubsub_project_id("tdg-dh-truehealth-core-nonprod", None).unwrap(),
            "tdg-dh-truehealth-core-nonprod"
        );
    }

    #[test]
    fn pubsub_project_id_falls_back_to_discovered_value() {
        assert_eq!(
            pubsub_project_id(" ", Some("local-project".to_string())).unwrap(),
            "local-project"
        );
    }

    #[test]
    fn pubsub_project_id_rejects_blank_value() {
        let error = pubsub_project_id("", None).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("google_cloud.project_id is required")
        );
    }
}
