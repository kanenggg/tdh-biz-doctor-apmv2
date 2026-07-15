use anyhow::Result;
use common_rs::{config::loader::load_conf_from_paths_with_default_dirs, twilio::TwilioConfig};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    // pub video: VideoProviderConfig,
    pub session: SessionConfig,
    pub twilio: TwilioConfig,
    pub booking: BookingConfig,
    pub debug: DebugConfig,
    pub redis: RedisConfig,
    pub google_cloud: GoogleCloud,
    #[serde(default)]
    pub rtdb_access: RtdbAccessConfig,
    pub follow_up: FollowUpConfig,
    #[serde(default)]
    pub doctor_service_projection: DoctorServiceProjectionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowUpConfig {
    pub max_days_ahead: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DoctorServiceProjectionConfig {
    /// When enabled, availability and reservation require a V2 service snapshot.
    #[serde(default)]
    pub require_v2_snapshot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub user: String,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub database_name: String,
}

impl DatabaseConfig {
    pub fn connection_url(&self) -> String {
        // URL-encode user and password to handle special characters
        let encoded_user = urlencoding::encode(&self.user);
        let encoded_password = urlencoding::encode(&self.password);
        format!(
            "postgresql://{}:{}@{}:{}/{}",
            encoded_user, encoded_password, self.host, self.port, self.database_name
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProviderConfig {
    pub provider_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub lead_time_seconds: i32,
    pub consultation_duration_minutes: i32,
    pub required_appointment_status: String,
    pub skip_chat_biz_unit_id: i32,
}

impl AppConfig {
    pub fn from(paths: &[std::path::PathBuf]) -> Result<Self> {
        load_conf_from_paths_with_default_dirs(
            paths,
            &[
                PathBuf::from("./config"),
                PathBuf::from("./consultation-rs/config"),
            ],
        )
    }

    pub fn lead_time_duration(&self) -> Duration {
        Duration::from_secs(self.session.lead_time_seconds as u64)
    }

    pub fn consultation_duration(&self) -> Duration {
        Duration::from_secs((self.session.consultation_duration_minutes * 60) as u64)
    }

    pub fn skip_chat_biz_unit_id(&self) -> i32 {
        self.session.skip_chat_biz_unit_id
    }

    pub fn required_appointment_status(&self) -> &str {
        &self.session.required_appointment_status
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookingConfig {
    /// TTL for the reservation hold in seconds (default: 900).
    pub reservation_ttl_seconds: i32,
    /// Duration of an instant consultation in seconds (default: 900).
    pub instant_consultation_duration_sec: i64,
    /// Ed25519 seed‖public-key as 128 hex chars for Paseto v4.public signing.
    // pub paseto_secret_key_hex: String,
    /// Full GCP KMS key resource name. Leave empty to use `LocalAesKms`.
    pub kms_key_name: String,
    /// PubSub topic name.
    pub pubsub_topic: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugConfig {
    pub always_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub host: String,
    pub user: String,
    pub password: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoogleCloud {
    pub project_id: String,
    /// PubSub emulator host (e.g. "localhost:8085"). Leave empty for production.
    pub pubsub_emulator_host: Option<String>,
    pub facial_upload_bucket: String,
    pub kms: KmsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RtdbAccessConfig {
    /// Firebase service account used by IAM Credentials signBlob. Empty disables
    /// RTDB token issuance without changing the existing Twilio session route.
    #[serde(default)]
    pub signing_service_account_email: String,
    #[serde(default = "default_rtdb_path_prefix")]
    pub path_prefix: String,
    #[serde(default = "default_rtdb_token_ttl_seconds")]
    pub token_ttl_seconds: i64,
}

fn default_rtdb_path_prefix() -> String {
    "consultations".to_string()
}

fn default_rtdb_token_ttl_seconds() -> i64 {
    300
}

impl Default for RtdbAccessConfig {
    fn default() -> Self {
        Self {
            signing_service_account_email: String::new(),
            path_prefix: default_rtdb_path_prefix(),
            token_ttl_seconds: default_rtdb_token_ttl_seconds(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KmsConfig {
    /// KMS key resource name for encrypting doctor summary notes
    pub doctor_note: String,
    /// KMS key resource name for encrypting / decrypting patient prescreen data
    pub prescreen: String,
}

impl RedisConfig {
    pub fn connection_url(&self) -> String {
        // URL-encode user and password to handle special characters like '!' in passwords
        let encoded_user = urlencoding::encode(&self.user);
        let encoded_password = urlencoding::encode(&self.password);
        format!(
            "redis://{}:{}@{}:{}",
            encoded_user, encoded_password, self.host, self.port
        )
    }
}
