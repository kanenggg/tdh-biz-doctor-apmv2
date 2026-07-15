use crate::common::tdh_protocol::{
    factial_verification::AddConsultationScreenshot, iam::user_identity::UserIdentity,
};
use axum::extract::multipart::Multipart;
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::post,
};

use crate::consultation::state::AppState;

#[utoipa::path(
    post,
    path = "/v2/consultation/facial-upload/{booking_id}",
    tag = "consultation",
    params(
        ("booking_id" = String, Path, description = "Booking ID / Appointment ID for facial screenshot upload")
    ),
    request_body(content = String, description = "Multipart form data with 'file' or 'image' field containing the image data", content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Facial screenshot uploaded successfully", body = AddConsultationScreenshot),
        (status = 400, description = "Bad request - malformed multipart data or no file provided"),
        (status = 401, description = "Unauthorized - user does not belong to this consultation", body = AddConsultationScreenshot),
        (status = 409, description = "Conflict - screenshot already uploaded", body = AddConsultationScreenshot),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("ApiKeyAuth" = [])
    )
)]
async fn facial_upload(
    Path(booking_id): Path<String>,
    Extension(user_identity): Extension<UserIdentity>,
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<AddConsultationScreenshot>, StatusCode> {
    while let Some(field) = multipart.next_field().await.map_err(|e| {
        tracing::warn!("Malformed facial upload multipart request: {}", e);
        StatusCode::BAD_REQUEST
    })? {
        let Some(name) = field.name() else {
            tracing::warn!("Facial upload multipart field is missing a name");
            return Err(StatusCode::BAD_REQUEST);
        };
        let name = name.to_string();

        if name == "file" || name == "image" {
            let file_data = field.bytes().await.map_err(|e| {
                tracing::warn!("Failed to read facial upload multipart field: {}", e);
                StatusCode::BAD_REQUEST
            })?;

            return state
                .facial_upload_service
                .upload(user_identity, file_data.to_vec(), &booking_id)
                .await
                .map(Json)
                .map_err(|e| {
                    tracing::error!("Facial upload error: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                });
        }
    }

    Err(StatusCode::BAD_REQUEST)
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/v2/consultation/facial-upload/{booking_id}",
        post(facial_upload),
    )
}
