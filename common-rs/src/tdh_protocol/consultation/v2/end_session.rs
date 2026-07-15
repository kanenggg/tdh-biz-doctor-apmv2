#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "__type")]
pub enum EndSessionResult {
    #[serde(rename = "EndSession.Success")]
    Success,
    #[serde(rename = "EndSession.SessionNotFound")]
    SessionNotFound,
    #[serde(rename = "EndSession.Unauthorized")]
    Unauthorized,
}
