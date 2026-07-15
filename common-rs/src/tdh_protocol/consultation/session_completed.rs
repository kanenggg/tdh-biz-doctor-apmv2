use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsultationSessionCompleted {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oidc_user_id: Option<String>,
    pub booking_id: String,
    pub biz_unit_id: i32,
}
