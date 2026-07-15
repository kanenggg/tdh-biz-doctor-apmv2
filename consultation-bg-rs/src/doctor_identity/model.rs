use rust_decimal::Decimal;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Localized {
    pub th: String,
    pub en: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profession {
    pub id: i32,
    pub name: Localized,
    pub abbr: Localized,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcademicPosition {
    pub id: i32,
    pub name: Localized,
    pub abbr: Localized,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkPlace {
    pub id: i32,
    pub name: Localized,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MedicalSchool {
    pub id: i32,
    pub name: Localized,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Specialty {
    pub id: i32,
    pub name: Localized,
    #[serde(default)]
    pub subspecialty: Option<Box<Specialty>>,
    pub medical_school: MedicalSchool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LanguageCode {
    Th,
    En,
}

impl LanguageCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Th => "th",
            Self::En => "en",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsultationChannel {
    Video,
    Voice,
    Chat,
}

impl ConsultationChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Voice => "voice",
            Self::Chat => "chat",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorConsultationConfig {
    pub channels: Vec<ConsultationChannel>,
    pub languages: Vec<LanguageCode>,
    pub duration_minutes: i32,
    pub fee_amount: String,
    #[serde(alias = "feeCurrency")]
    pub currency: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorServiceConfig {
    pub channels: Vec<ConsultationChannel>,
    pub languages: Vec<LanguageCode>,
    pub duration_minutes: i32,
    pub fee_amount: Decimal,
    pub currency: String,
}

impl DoctorServiceConfig {
    fn validate(&self) -> Result<(), DoctorProfileEventValidationError> {
        if self.channels.is_empty() {
            return Err(
                DoctorProfileEventValidationError::InvalidConsultationConfig(
                    "channels must be a non-empty list of video, voice, or chat".to_string(),
                ),
            );
        }
        if self.languages.is_empty() {
            return Err(
                DoctorProfileEventValidationError::InvalidConsultationConfig(
                    "languages must be a non-empty list of th or en".to_string(),
                ),
            );
        }
        if !matches!(self.duration_minutes, 15 | 25 | 50) {
            return Err(
                DoctorProfileEventValidationError::InvalidConsultationConfig(
                    "durationMinutes must be one of 15, 25, or 50".to_string(),
                ),
            );
        }
        if self.fee_amount.is_sign_negative() || self.fee_amount.scale() != 2 {
            return Err(
                DoctorProfileEventValidationError::InvalidConsultationConfig(
                    "fee must be nonnegative with exactly two fractional digits".to_string(),
                ),
            );
        }
        if self.currency.trim().is_empty() {
            return Err(
                DoctorProfileEventValidationError::InvalidConsultationConfig(
                    "currency must be non-empty".to_string(),
                ),
            );
        }
        Ok(())
    }
}

impl DoctorConsultationConfig {
    fn into_service_config(self) -> Result<DoctorServiceConfig, DoctorProfileEventValidationError> {
        let fee_amount = self.fee_amount.parse::<Decimal>().map_err(|_| {
            DoctorProfileEventValidationError::InvalidConsultationConfig(
                "feeAmount must be a decimal string with exactly two fractional digits".to_string(),
            )
        })?;
        let config = DoctorServiceConfig {
            channels: self.channels,
            languages: self.languages,
            duration_minutes: self.duration_minutes,
            fee_amount,
            currency: self.currency,
        };
        config.validate()?;
        Ok(config)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "__type", rename_all = "PascalCase")]
pub enum DoctorProfileEvent {
    #[serde(rename_all = "camelCase")]
    DoctorProfileApproved {
        event_id: String,
        doctor_id: Uuid,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        department_id: i32,
        department: Localized,
        counseling_areas: Vec<Localized>,
        is_active: bool,
        profession: Profession,
        specialty: Specialty,
        work_place: Vec<WorkPlace>,
        academic_position: AcademicPosition,
        first_name: Localized,
        last_name: Localized,
        profile_image_url: String,
        doctor_fee: i32,
        doctor_fee_currency: String,
        languages: Vec<LanguageCode>,
        duration_minutes: i32,
        channels: Vec<ConsultationChannel>,
        approved_at: i64,
        occurred_at: i64,
        #[serde(default)]
        schema_version: Option<i32>,
        #[serde(default)]
        profile_version: Option<i64>,
        #[serde(default)]
        consultation_config: Option<DoctorConsultationConfig>,
    },
    #[serde(rename_all = "camelCase")]
    DoctorProfileDeactivated {
        event_id: String,
        doctor_id: Uuid,
        doctor_account_id: i32,
        doctor_profile_id: i32,
        reason: String,
        deactivated_at: i64,
        occurred_at: i64,
        #[serde(default)]
        schema_version: Option<i32>,
        #[serde(default)]
        profile_version: Option<i64>,
    },
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DoctorProfileEventValidationError {
    #[error("unsupported schemaVersion {0}")]
    UnsupportedSchemaVersion(i32),
    #[error("profileVersion must be a positive integer when supplied")]
    InvalidProfileVersion,
    #[error("invalid consultationConfig: {0}")]
    InvalidConsultationConfig(String),
    #[error("top-level committed configuration contradicts consultationConfig")]
    ContradictoryConsultationConfig,
}

impl DoctorProfileEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::DoctorProfileApproved { .. } => "DoctorProfileApproved",
            Self::DoctorProfileDeactivated { .. } => "DoctorProfileDeactivated",
        }
    }

    pub fn profile_version(&self) -> Option<i64> {
        match self {
            Self::DoctorProfileApproved {
                profile_version, ..
            }
            | Self::DoctorProfileDeactivated {
                profile_version, ..
            } => *profile_version,
        }
    }

    pub fn occurred_at(&self) -> i64 {
        match self {
            Self::DoctorProfileApproved { occurred_at, .. }
            | Self::DoctorProfileDeactivated { occurred_at, .. } => *occurred_at,
        }
    }

    pub fn committed_service_config(
        &self,
    ) -> Result<DoctorServiceConfig, DoctorProfileEventValidationError> {
        match self {
            Self::DoctorProfileApproved {
                channels,
                languages,
                duration_minutes,
                doctor_fee,
                doctor_fee_currency,
                ..
            } => {
                let mut fee_amount = Decimal::from(*doctor_fee);
                fee_amount.rescale(2);
                let config = DoctorServiceConfig {
                    channels: channels.clone(),
                    languages: languages.clone(),
                    duration_minutes: *duration_minutes,
                    fee_amount,
                    currency: doctor_fee_currency.clone(),
                };
                config.validate()?;
                Ok(config)
            }
            Self::DoctorProfileDeactivated { .. } => Err(
                DoctorProfileEventValidationError::InvalidConsultationConfig(
                    "deactivated events do not carry service configuration".to_string(),
                ),
            ),
        }
    }

    pub fn validate(
        &self,
        allowed_schema_versions: &[i32],
    ) -> Result<(), DoctorProfileEventValidationError> {
        let (schema_version, profile_version) = match self {
            Self::DoctorProfileApproved {
                schema_version,
                profile_version,
                consultation_config,
                ..
            } => {
                let committed_config = self.committed_service_config()?;
                if let Some(extension) = consultation_config.clone() {
                    if extension.into_service_config()? != committed_config {
                        return Err(
                            DoctorProfileEventValidationError::ContradictoryConsultationConfig,
                        );
                    }
                }
                (*schema_version, *profile_version)
            }
            Self::DoctorProfileDeactivated {
                schema_version,
                profile_version,
                ..
            } => (*schema_version, *profile_version),
        };

        if let Some(schema_version) = schema_version {
            if !allowed_schema_versions.contains(&schema_version) {
                return Err(DoctorProfileEventValidationError::UnsupportedSchemaVersion(
                    schema_version,
                ));
            }
        }
        if profile_version.is_some_and(|version| version <= 0) {
            return Err(DoctorProfileEventValidationError::InvalidProfileVersion);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_origin_main_fixture_projects_top_level_service_configuration() {
        let event: DoctorProfileEvent = serde_json::from_str(include_str!(
            "../../../fixtures/doctor_profile_approved_origin_main.json"
        ))
        .expect("committed DoctorApp origin/main fixture must deserialize");

        event
            .validate(&[2])
            .expect("committed DoctorApp origin/main fixture must validate");
        let config = event.committed_service_config().expect("approved config");

        assert_eq!(config.fee_amount.to_string(), "650.00");
        assert_eq!(config.currency, "THB");
        assert_eq!(config.duration_minutes, 15);
        assert_eq!(
            config.channels,
            vec![ConsultationChannel::Voice, ConsultationChannel::Chat]
        );
        assert_eq!(config.languages, vec![LanguageCode::Th, LanguageCode::En]);
    }

    #[test]
    fn versioned_extension_must_match_the_committed_top_level_config() {
        let mut payload: serde_json::Value = serde_json::from_str(include_str!(
            "../../../fixtures/doctor_profile_approved_origin_main.json"
        ))
        .unwrap();
        payload["schemaVersion"] = serde_json::json!(2);
        payload["profileVersion"] = serde_json::json!(7);
        payload["consultationConfig"] = serde_json::json!({
            "channels": ["voice", "chat"], "languages": ["th", "en"],
            "durationMinutes": 15, "feeAmount": "650.00", "currency": "THB"
        });
        let matching: DoctorProfileEvent = serde_json::from_value(payload.clone()).unwrap();
        assert!(matching.validate(&[2]).is_ok());

        payload["consultationConfig"]["feeAmount"] = serde_json::json!("651.00");
        let contradictory: DoctorProfileEvent = serde_json::from_value(payload).unwrap();
        assert_eq!(
            contradictory.validate(&[2]),
            Err(DoctorProfileEventValidationError::ContradictoryConsultationConfig)
        );
    }

    #[test]
    fn deactivated_committed_event_remains_deserializable() {
        let event: DoctorProfileEvent = serde_json::from_value(serde_json::json!({
            "__type": "DoctorProfileDeactivated", "eventId": "evt-deactivated",
            "doctorId": Uuid::nil(), "doctorAccountId": 10, "doctorProfileId": 20,
            "reason": "license expired", "deactivatedAt": 1718668800, "occurredAt": 1718668800
        }))
        .expect("committed deactivation payload should deserialize");
        assert!(event.validate(&[2]).is_ok());
    }
}
