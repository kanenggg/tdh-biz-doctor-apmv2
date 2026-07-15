use std::sync::Arc;

use anyhow::Result;
use common_rs::twilio::TwilioConfig;
use deadpool_redis::Runtime;
use google_cloud_storage::client::Storage as GcsStorage;
use sqlx::PgPool;

use crate::common::{TwilioClient, TwilioClientImpl};
use crate::infra::event::EventPublisher;
use crate::sys::config::AppConfig;

pub struct Infrastructure {
    pub db_pool: PgPool,
    pub redis_pool: deadpool_redis::Pool,
    pub event_publisher: Arc<dyn EventPublisher>,
    pub gcs_client: GcsStorage,
    pub twilio_client: Arc<dyn TwilioClient>,
    pub config: AppConfig,
}

impl Infrastructure {
    pub async fn new(config: AppConfig) -> Result<Self> {
        tracing::info!("Connecting to database...");
        let db_pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(&config.database.connection_url())
            .await
            .map_err(|e| {
                tracing::error!("Failed to connect to database: {}", e);
                e
            })?;
        tracing::info!("Database connected");

        tracing::info!("Connecting to Redis...");
        let redis_pool = deadpool_redis::Config::from_url(config.redis.connection_url())
            .create_pool(Some(Runtime::Tokio1))
            .map_err(|e| {
                tracing::error!("Failed to create Redis pool: {}", e);
                e
            })?;
        tracing::info!("Redis pool created");

        let twilio_config = TwilioConfig {
            base_url: config.twilio.base_url.clone(),
            account_sid: config.twilio.account_sid.clone(),
            api_key_sid: config.twilio.api_key_sid.clone(),
            api_key_secret: config.twilio.api_key_secret.clone(),
            auth_token: config.twilio.auth_token.clone(),
            callback_url: config.twilio.callback_url.clone(),
            video_base_url: config.twilio.video_base_url.clone(),
            chat_base_url: config.twilio.chat_base_url.clone(),
        };
        let twilio_client = Arc::new(TwilioClientImpl::new(twilio_config));

        let gcs_client = GcsStorage::builder().build().await?;

        Ok(Self {
            db_pool,
            redis_pool,
            event_publisher: Arc::new(crate::infra::event::NoOpEventPublisher),
            gcs_client,
            twilio_client,
            config,
        })
    }

    pub fn with_event_publisher(mut self, publisher: Arc<dyn EventPublisher>) -> Self {
        self.event_publisher = publisher;
        self
    }
}
