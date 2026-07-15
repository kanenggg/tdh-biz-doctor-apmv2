//! Common compatibility types for tdh-protocol with serde JSON support.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ===== Simple Enums with serde support =====

/// Consultation channel with JSON compatibility
/// Accepts "video", "VIDEO", "Video" etc. (case-insensitive) to handle Java service output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, ToSchema)]
pub enum ConsultationChannel {
    Video,
    Chat,
    Voice,
}

impl Serialize for ConsultationChannel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            ConsultationChannel::Video => "video",
            ConsultationChannel::Chat => "chat",
            ConsultationChannel::Voice => "voice",
        };
        serializer.serialize_str(s)
    }
}

impl<'de> Deserialize<'de> for ConsultationChannel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "video" => Ok(ConsultationChannel::Video),
            "chat" => Ok(ConsultationChannel::Chat),
            "voice" => Ok(ConsultationChannel::Voice),
            _ => Err(serde::de::Error::unknown_variant(
                &s,
                &["video", "chat", "voice"],
            )),
        }
    }
}

/// Booking type with JSON compatibility
///
/// Uses __type tag for backward compatibility with existing JSON format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(tag = "__type")]
pub enum BookingType {
    #[serde(
        rename = "scheduled",
        alias = "Scheduled",
        alias = "SCHEDULED",
        alias = "Schedule"
    )]
    Scheduled,
    #[serde(rename = "instant", alias = "Instant", alias = "INSTANT")]
    Instant,
}

// ===== PatientIdentity with serde support =====

/// Patient identity with JSON compatibility
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PatientIdentity {
    pub account_id: i32,
    pub user_profile_id: i32,
    pub tenant_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oidc_user_id: Option<String>,
}

// ===== Tests =====

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_patient_identity_serialization() {
        let patient = PatientIdentity {
            account_id: 123,
            user_profile_id: 456,
            tenant_id: 1,
            oidc_user_id: Some("abc123".to_string()),
        };

        let json = serde_json::to_string(&patient).unwrap();
        assert!(json.contains("\"accountId\":123"));
        assert!(json.contains("\"userProfileId\":456"));
        assert!(json.contains("\"oidcUserId\":\"abc123\""));
    }

    #[test]
    fn test_patient_identity_deserialization() {
        let json = r#"{"accountId":123,"userProfileId":456,"tenantId":1}"#;
        let patient: PatientIdentity = serde_json::from_str(json).unwrap();
        assert_eq!(patient.account_id, 123);
        assert_eq!(patient.user_profile_id, 456);
        assert_eq!(patient.tenant_id, 1);
        assert!(patient.oidc_user_id.is_none());
    }

    #[test]
    fn test_consultation_channel_serialization() {
        let channel = ConsultationChannel::Video;
        let json = serde_json::to_string(&channel).unwrap();
        assert_eq!(json, "\"video\"");
    }

    #[test]
    fn test_consultation_channel_deserialization() {
        let channel: ConsultationChannel = serde_json::from_str("\"video\"").unwrap();
        assert_eq!(channel, ConsultationChannel::Video);
    }

    #[test]
    fn test_booking_type_serialization() {
        let booking_type = BookingType::Scheduled;
        let json = serde_json::to_string(&booking_type).unwrap();
        assert_eq!(json, "{\"__type\":\"scheduled\"}");
    }

    #[test]
    fn test_booking_type_schedule_alias() {
        // Test that "Schedule" (without trailing 'd') deserializes correctly
        // This is the value sent by the Java service
        let json = r#"{"__type":"Schedule"}"#;
        let booking_type: BookingType = serde_json::from_str(json).unwrap();
        assert_eq!(booking_type, BookingType::Scheduled);
    }
}
