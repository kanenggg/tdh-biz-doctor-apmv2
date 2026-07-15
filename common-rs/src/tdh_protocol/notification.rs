//! Compatibility layer for tdh-protocol notification types with serde JSON support.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

/// Notification types with JSON compatibility
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum NotificationType {
    Appointment,
    Consultation,
    System,
    Broadcast,
}

/// Notification payload with JSON compatibility
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPayload {
    #[serde(rename = "type")]
    pub notification_type: NotificationType,
    #[serde(
        rename = "doctorAccountIds",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub doctor_account_ids: Option<Vec<i32>>,
    pub title: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<HashMap<String, serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(
        rename = "scheduledAt",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[schema(value_type = i64, format = "int64")]
    pub scheduled_at: Option<Timestamp>,
}

/// Payload for Cloud Tasks execution with JSON compatibility
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledNotificationTask {
    pub notification: NotificationPayload,
    #[serde(rename = "originalScheduleTime")]
    #[schema(value_type = String, format = DateTime)]
    pub original_schedule_time: Timestamp,
    pub chain_count: u32,
}

/// Individual Pub/Sub message with JSON compatibility
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PubsubMessage {
    /// Base64-encoded data
    pub data: String,
    #[serde(rename = "messageId")]
    pub message_id: String,
    #[serde(rename = "publishTime")]
    pub publish_time: String,
    #[serde(default)]
    pub attributes: HashMap<String, String>,
}

#[cfg(feature = "gcp-pubsub")]
impl PubsubMessage {
    /// Decode the base64-encoded data field
    pub fn decode_data(&self) -> Result<Vec<u8>, base64::DecodeError> {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.decode(&self.data)
    }

    /// Decode the base64-encoded data field as a UTF-8 string
    #[allow(dead_code)]
    pub fn decode_data_utf8(&self) -> Result<String, anyhow::Error> {
        let bytes = self.decode_data()?;
        Ok(String::from_utf8(bytes)?)
    }
}

/// Pub/Sub push message received from webhook with JSON compatibility
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PubsubPushMessage {
    pub message: PubsubMessage,
    pub subscription: String,
}

// ===== Validation methods =====

impl NotificationPayload {
    /// Grace period for clock drift when validating scheduled times
    const GRACE_PERIOD_SECS: i64 = 300;
    /// Maximum scheduling horizon (1 year)
    const MAX_SCHEDULE_DAYS: i64 = 365;

    /// Validate the scheduled time is within acceptable bounds
    pub fn validate_scheduled_time(&self) -> Result<(), anyhow::Error> {
        if let Some(scheduled_at) = self.scheduled_at {
            let now = Timestamp::now();

            let grace_period = jiff::Span::new().seconds(Self::GRACE_PERIOD_SECS);
            if scheduled_at < now.checked_sub(grace_period).unwrap() {
                return Err(anyhow::anyhow!("Scheduled time is too far in the past"));
            }

            let max_schedule = jiff::Span::new().hours(Self::MAX_SCHEDULE_DAYS * 24);
            if scheduled_at > now.checked_add(max_schedule).unwrap() {
                return Err(anyhow::anyhow!(
                    "Scheduled time exceeds maximum allowed duration of 1 year"
                ));
            }
        }
        Ok(())
    }
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_type_serialization() {
        let nt = NotificationType::Appointment;
        let json = serde_json::to_string(&nt).unwrap();
        assert_eq!(json, "\"appointment\"");
    }

    #[test]
    fn test_notification_type_deserialization() {
        let nt: NotificationType = serde_json::from_str("\"system\"").unwrap();
        assert_eq!(nt, NotificationType::System);
    }

    #[test]
    fn test_notification_payload_serialization() {
        let payload = NotificationPayload {
            notification_type: NotificationType::Consultation,
            doctor_account_ids: Some(vec![1, 2, 3]),
            title: "Test".to_string(),
            body: "Test body".to_string(),
            data: None,
            category: Some("test".to_string()),
            scheduled_at: None,
        };

        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains("\"type\":\"consultation\""));
        assert!(json.contains("\"doctorAccountIds\":[1,2,3]"));
        assert!(json.contains("\"category\":\"test\""));
    }

    #[test]
    #[cfg(feature = "gcp-pubsub")]
    fn test_pubsub_message_decode() {
        use base64::Engine;
        let msg = PubsubMessage {
            data: base64::engine::general_purpose::STANDARD.encode("hello world"),
            message_id: "123".to_string(),
            publish_time: "2024-01-01T00:00:00Z".to_string(),
            attributes: HashMap::new(),
        };

        let decoded = msg.decode_data().unwrap();
        assert_eq!(decoded, b"hello world");
    }

    #[test]
    fn test_scheduled_notification_task_serialization() {
        let task = ScheduledNotificationTask {
            notification: NotificationPayload {
                notification_type: NotificationType::Appointment,
                doctor_account_ids: None,
                title: "Test".to_string(),
                body: "Test".to_string(),
                data: None,
                category: None,
                scheduled_at: None,
            },
            original_schedule_time: Timestamp::now(),
            chain_count: 1,
        };

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("\"originalScheduleTime\""));
        assert!(json.contains("\"chainCount\":1"));
    }

    #[test]
    fn test_validate_scheduled_time() {
        let valid_payload = NotificationPayload {
            notification_type: NotificationType::System,
            doctor_account_ids: None,
            title: "Test".to_string(),
            body: "Test".to_string(),
            data: None,
            category: None,
            scheduled_at: Some(
                Timestamp::now()
                    .checked_add(jiff::Span::new().hours(1))
                    .unwrap(),
            ),
        };

        assert!(valid_payload.validate_scheduled_time().is_ok());
    }

    #[test]
    fn test_validate_scheduled_time_past() {
        let past_payload = NotificationPayload {
            notification_type: NotificationType::System,
            doctor_account_ids: None,
            title: "Test".to_string(),
            body: "Test".to_string(),
            data: None,
            category: None,
            scheduled_at: Some(
                Timestamp::now()
                    .checked_sub(jiff::Span::new().minutes(10))
                    .unwrap(),
            ),
        };

        assert!(past_payload.validate_scheduled_time().is_err());
    }

    #[test]
    fn test_validate_scheduled_time_too_far() {
        let far_payload = NotificationPayload {
            notification_type: NotificationType::System,
            doctor_account_ids: None,
            title: "Test".to_string(),
            body: "Test".to_string(),
            data: None,
            category: None,
            scheduled_at: Some(
                Timestamp::now()
                    .checked_add(jiff::Span::new().hours(400 * 24))
                    .unwrap(),
            ),
        };

        assert!(far_payload.validate_scheduled_time().is_err());
    }
}
