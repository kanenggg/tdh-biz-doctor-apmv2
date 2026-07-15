use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialUserIdentity {
    pub account_id: u64,
    pub user_profile_id: u64,
    pub tenant_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oidc_user_id: Option<String>,
}
