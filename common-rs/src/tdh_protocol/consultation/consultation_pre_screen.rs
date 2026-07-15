use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationPreScreen {
    pub symptom: String,
    pub duration: i32,
    #[serde(alias = "duration_unit")]
    pub duration_unit: String,
    pub attachments: Vec<String>,
    pub allergies: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_duration_unit_as_camel_case() {
        let prescreen = ConsultationPreScreen {
            symptom: "headache".to_string(),
            duration: 3,
            duration_unit: "day".to_string(),
            attachments: vec!["a.jpg".to_string()],
            allergies: vec!["pollen".to_string()],
        };

        let json = serde_json::to_value(prescreen).expect("serialize prescreen");

        assert_eq!(json["durationUnit"], "day");
        assert!(json.get("duration_unit").is_none());
    }
}
