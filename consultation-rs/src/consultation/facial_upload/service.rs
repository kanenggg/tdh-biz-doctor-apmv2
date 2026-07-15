use std::sync::Arc;

use crate::common::tdh_protocol::{
    factial_verification::AddConsultationScreenshot,
    iam::user_identity::{AccountType, UserIdentity},
};
use google_cloud_storage::client::Storage as GcsClient;

use crate::consultation::facial_upload::repo::FacialUploadRepoPsql;

#[derive(Clone)]
pub struct FacialUploadService {
    repo: Arc<FacialUploadRepoPsql>,
    gcs_client: GcsClient,
    facial_upload_bucket: String,
}

impl FacialUploadService {
    pub fn new(
        repo: Arc<FacialUploadRepoPsql>,
        gcs_client: GcsClient,
        facial_upload_bucket: String,
    ) -> Self {
        Self {
            repo,
            gcs_client,
            facial_upload_bucket,
        }
    }

    pub async fn upload(
        &self,
        user_id: UserIdentity,
        file: Vec<u8>,
        booking_id: &str,
    ) -> Result<AddConsultationScreenshot, anyhow::Error> {
        let role = match user_id.account_type {
            AccountType::Patient => "patient",
            AccountType::Doctor => "doctor",
        };

        let appointment_id = self
            .repo
            .get_appointment_detail(booking_id, role, user_id.user_profile_id as i64)
            .await?;

        let Some(appointment_id) = appointment_id else {
            return Ok(AddConsultationScreenshot::ConsultationNotFound);
        };

        let object_name = format!("test/{}/{}_success", booking_id, role);

        let write_object_result = self
            .gcs_client
            .write_object(
                &self.facial_upload_bucket,
                object_name.clone(),
                bytes::Bytes::from(file),
            )
            .send_buffered()
            .await?;

        tracing::info!(
            "write_object_result {}: {:?}",
            booking_id,
            write_object_result
        );

        // Insert record into appointment_facial_upload table
        let object_url = format!("gs://{}/{}", self.facial_upload_bucket, object_name);
        self.repo
            .insert_facial_upload(
                &appointment_id,
                user_id.user_profile_id as i32,
                user_id.account_id as i32,
                &object_url,
            )
            .await?;

        Ok(AddConsultationScreenshot::UploadSuccess)
    }
}
