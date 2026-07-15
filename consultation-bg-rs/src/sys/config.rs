use common_rs::config::loader::load_conf_from_paths_with_default_dirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub payment: PaymentConfig,
    #[serde(default)]
    pub payment_confirm: PaymentConfirmConfig,
    pub google_cloud: GoogleCloudConfig,
    pub consultation_event: ConsultationEventConfig,
    #[serde(default)]
    pub event_outbox: EventOutboxConfig,
    #[serde(default)]
    pub appointment_hold_expiry: AppointmentHoldExpiryConfig,
    #[serde(default)]
    pub doctor_projection_sync: DoctorProjectionSyncConfig,
    #[serde(default)]
    pub doctor_projection: DoctorProjectionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DatabaseConfig {
    pub user: String,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub database_name: String,
}

impl DatabaseConfig {
    pub fn connection_url(&self) -> String {
        let encoded_user = urlencoding::encode(&self.user);
        let encoded_password = urlencoding::encode(&self.password);
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            encoded_user, encoded_password, self.host, self.port, self.database_name
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PaymentConfig {
    /// Ed25519 public key as hex string for PASETO v2.public verification.
    #[serde(alias = "secret_key")]
    pub public_key: String,
}

/// Safe by default: a projection-only deployment must never expose a route
/// that calls the gated Hold-cutover payment function.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct PaymentConfirmConfig {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct GoogleCloudConfig {
    pub project_id: String,
    pub pubsub_emulator_host: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ConsultationEventConfig {
    pub pubsub_topic: String,
}

impl ConsultationEventConfig {
    fn validate(&self) -> anyhow::Result<()> {
        if self.pubsub_topic.trim().is_empty() {
            anyhow::bail!("consultation_event.pubsub_topic is required")
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct EventOutboxConfig {
    #[serde(default = "default_event_outbox_enabled")]
    pub enabled: bool,
    #[serde(default = "default_event_outbox_poll_interval_seconds")]
    pub poll_interval_seconds: u64,
    #[serde(default = "default_event_outbox_batch_size")]
    pub batch_size: i64,
    #[serde(default = "default_event_outbox_lock_seconds")]
    pub lock_seconds: i32,
}

fn default_event_outbox_enabled() -> bool {
    true
}

fn default_event_outbox_poll_interval_seconds() -> u64 {
    5
}

fn default_event_outbox_batch_size() -> i64 {
    50
}

fn default_event_outbox_lock_seconds() -> i32 {
    60
}

impl Default for EventOutboxConfig {
    fn default() -> Self {
        Self {
            enabled: default_event_outbox_enabled(),
            poll_interval_seconds: default_event_outbox_poll_interval_seconds(),
            batch_size: default_event_outbox_batch_size(),
            lock_seconds: default_event_outbox_lock_seconds(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AppointmentHoldExpiryConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_hold_expiry_poll_interval_seconds")]
    pub poll_interval_seconds: u64,
    #[serde(default = "default_hold_expiry_batch_size")]
    pub batch_size: i32,
}
fn default_hold_expiry_poll_interval_seconds() -> u64 {
    30
}
fn default_hold_expiry_batch_size() -> i32 {
    50
}
impl Default for AppointmentHoldExpiryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            poll_interval_seconds: default_hold_expiry_poll_interval_seconds(),
            batch_size: default_hold_expiry_batch_size(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DoctorProjectionSyncConfig {
    /// Allowed Google OAuth access-token service account emails for direct
    /// DoctorApp -> APM projection sync. Empty means the sync endpoint is disabled.
    #[serde(default)]
    pub allowed_service_account_emails: Vec<String>,
    #[serde(default = "default_google_tokeninfo_url")]
    pub tokeninfo_url: String,
}

fn default_google_tokeninfo_url() -> String {
    "https://oauth2.googleapis.com/tokeninfo".to_string()
}

impl Default for DoctorProjectionSyncConfig {
    fn default() -> Self {
        Self {
            allowed_service_account_emails: Vec::new(),
            tokeninfo_url: default_google_tokeninfo_url(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct DoctorProjectionConfig {
    /// V2 event schemas accepted by this consumer. Keep this allowlist explicit
    /// during rollout so incompatible producer changes fail retryably.
    #[serde(default = "default_allowed_schema_versions")]
    pub allowed_schema_versions: Vec<i32>,
}

fn default_allowed_schema_versions() -> Vec<i32> {
    vec![2]
}

impl Default for DoctorProjectionConfig {
    fn default() -> Self {
        Self {
            allowed_schema_versions: default_allowed_schema_versions(),
        }
    }
}

impl AppConfig {
    pub fn from(paths: &[std::path::PathBuf]) -> anyhow::Result<Self> {
        let config: Self = load_conf_from_paths_with_default_dirs(
            paths,
            &[
                PathBuf::from("./config"),
                PathBuf::from("./consultation-bg-rs/config"),
            ],
        )?;
        config.consultation_event.validate()?;
        if config.event_outbox.poll_interval_seconds == 0
            || config.event_outbox.batch_size <= 0
            || config.event_outbox.lock_seconds <= 0
        {
            anyhow::bail!(
                "event_outbox poll_interval_seconds, batch_size, and lock_seconds must be positive"
            )
        }
        if config.appointment_hold_expiry.poll_interval_seconds == 0
            || config.appointment_hold_expiry.batch_size <= 0
        {
            anyhow::bail!(
                "appointment_hold_expiry poll_interval_seconds and batch_size must be positive"
            )
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn database_connection_url_percent_encodes_credentials() {
        let config = DatabaseConfig {
            user: "doctor user".to_string(),
            password: "p@ss/word".to_string(),
            host: "db.example.test".to_string(),
            port: 5432,
            database_name: "consultation".to_string(),
        };

        assert_eq!(
            config.connection_url(),
            "postgresql://doctor%20user:p%40ss%2Fword@db.example.test:5432/consultation"
        );
    }

    #[test]
    fn app_config_defaults_doctor_projection_sync_when_missing() {
        let config: AppConfig = serde_json::from_value(json!({
            "server": { "host": "127.0.0.1", "port": 8080 },
            "database": {
                "user": "user",
                "password": "password",
                "host": "localhost",
                "port": 5432,
                "database_name": "consultation"
            },
            "payment": { "public_key": "public-key" },
            "google_cloud": { "project_id": "project", "pubsub_emulator_host": null },
            "consultation_event": { "pubsub_topic": "consultation-event-v1" }
        }))
        .expect("missing optional sections should use defaults");

        assert!(
            config
                .doctor_projection_sync
                .allowed_service_account_emails
                .is_empty()
        );
        assert_eq!(
            config.doctor_projection_sync.tokeninfo_url,
            "https://oauth2.googleapis.com/tokeninfo"
        );
        assert!(config.event_outbox.enabled);
        assert_eq!(config.doctor_projection.allowed_schema_versions, vec![2]);
    }
}
