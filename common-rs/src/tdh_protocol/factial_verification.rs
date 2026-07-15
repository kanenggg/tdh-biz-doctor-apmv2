#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "__type")]
pub enum AddConsultationScreenshot {
    #[serde(rename = "AddConsultationScreenshot.UploadSuccess")]
    UploadSuccess,
    #[serde(rename = "AddConsultationScreenshot.ScreenshotAlreadyUploaded")]
    ScreenshotAlreadyUploaded,
    #[serde(rename = "AddConsultationScreenshot.ConsultationNotFound")]
    ConsultationNotFound,
    #[serde(rename = "AddConsultationScreenshot.Unauthorized")]
    Unauthorized,
}
