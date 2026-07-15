use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Patient identity with JSON compatibility
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PartialUserIdentity {
    pub account_id: i32,
    pub user_profile_id: i32,
    pub tenant_id: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oidc_user_id: Option<String>,
}
