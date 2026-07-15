#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "__type")]
pub enum PatientIdVerificationResult {
    #[serde(rename = "PatientIdVerificationResult.Success")]
    Success,
    #[serde(rename = "PatientIdVerificationResult.Endded")]
    SesssionEnded,
    /// Authentication fail, or resource not found
    #[serde(rename = "PatientIdVerificationResult.SessionNotFound")]
    SessionNotFound,
    /// Sesssion status is not permitted for operation
    #[serde(rename = "PatientIdVerificationResult.SessionStatusNotPermitted")]
    SessionStatusNotPermitted,
}
