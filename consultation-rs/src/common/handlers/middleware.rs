use crate::common::tdh_protocol::iam::user_identity::UserIdentity;
use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

pub async fn auth_middleware(headers: HeaderMap, mut request: Request, next: Next) -> Response {
    let user_identity = extract_user_identity_from_headers(&headers);

    match user_identity {
        Ok(identity) => {
            request.extensions_mut().insert(identity);
            next.run(request).await
        }
        Err(e) => (StatusCode::UNAUTHORIZED, e).into_response(),
    }
}

fn extract_user_identity_from_headers(headers: &HeaderMap) -> Result<UserIdentity, String> {
    let header_value = headers
        .get("tdh-sec-iam-user-identity")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| "Missing tdh-sec-iam-user-identity header".to_string())?;

    serde_json::from_str(header_value)
        .map_err(|e| format!("Failed to parse tdh-sec-iam-user-identity header: {}", e))
}
