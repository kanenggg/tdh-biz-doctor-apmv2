use axum::{
    extract::rejection::JsonRejection,
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde_json::json;
use tracing::{Level, Span};

const TRACE_PARENT_HEADER: &str = "traceparent";
const TRACE_ID_HEADER: &str = "x-trace-id";
const SPAN_ID_HEADER: &str = "x-span-id";

pub enum TraceError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    InternalError(String),
}

impl TraceError {
    fn status_code(&self) -> StatusCode {
        match self {
            TraceError::BadRequest(_) => StatusCode::BAD_REQUEST,
            TraceError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            TraceError::Forbidden(_) => StatusCode::FORBIDDEN,
            TraceError::NotFound(_) => StatusCode::NOT_FOUND,
            TraceError::Conflict(_) => StatusCode::CONFLICT,
            TraceError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_message(&self) -> &str {
        match self {
            TraceError::BadRequest(msg) => msg,
            TraceError::Unauthorized(msg) => msg,
            TraceError::Forbidden(msg) => msg,
            TraceError::NotFound(msg) => msg,
            TraceError::Conflict(msg) => msg,
            TraceError::InternalError(msg) => msg,
        }
    }

    fn error_type(&self) -> &str {
        match self {
            TraceError::BadRequest(_) => "BAD_REQUEST",
            TraceError::Unauthorized(_) => "UNAUTHORIZED",
            TraceError::Forbidden(_) => "FORBIDDEN",
            TraceError::NotFound(_) => "NOT_FOUND",
            TraceError::Conflict(_) => "CONFLICT",
            TraceError::InternalError(_) => "INTERNAL_ERROR",
        }
    }

    fn trace_context(&self) -> TraceContext {
        let span = Span::current();

        let trace_id = span
            .id()
            .map(|id| id.into_u64().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let span_id = span
            .id()
            .map(|id| id.into_u64().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        let trace_parent = if trace_id != "unknown" && span_id != "unknown" {
            Some(format!("00-{}-{}-01", trace_id, span_id))
        } else {
            None
        };

        TraceContext {
            trace_id,
            span_id,
            trace_parent,
        }
    }
}

struct TraceContext {
    trace_id: String,
    span_id: String,
    trace_parent: Option<String>,
}

impl axum::response::IntoResponse for TraceError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let error_type = self.error_type();
        let error_message = self.error_message();
        let trace_context = self.trace_context();

        let mut headers = HeaderMap::new();

        if let Some(trace_parent) = trace_context.trace_parent {
            if let Ok(value) = HeaderValue::from_str(&trace_parent) {
                headers.insert(TRACE_PARENT_HEADER, value);
            }
        }

        if let Ok(trace_id) = HeaderValue::from_str(&trace_context.trace_id) {
            headers.insert(TRACE_ID_HEADER, trace_id);
        }

        if let Ok(span_id) = HeaderValue::from_str(&trace_context.span_id) {
            headers.insert(SPAN_ID_HEADER, span_id);
        }

        let body = Json(json!({
            "error": {
                "type": error_type,
                "message": error_message,
                "trace_id": trace_context.trace_id,
                "span_id": trace_context.span_id,
            }
        }));

        tracing::event!(
            target: "http_error",
            Level::ERROR,
            error_type = error_type,
            error_message = error_message,
            trace_id = %trace_context.trace_id,
            span_id = %trace_context.span_id,
            status_code = %status.as_u16(),
            "HTTP error response"
        );

        (status, headers, body).into_response()
    }
}

impl From<&str> for TraceError {
    fn from(msg: &str) -> Self {
        TraceError::InternalError(msg.to_string())
    }
}

impl From<String> for TraceError {
    fn from(msg: String) -> Self {
        TraceError::InternalError(msg)
    }
}

pub fn bad_request() -> TraceError {
    TraceError::BadRequest("Bad request".to_string())
}

pub fn bad_request_msg(msg: impl Into<String>) -> TraceError {
    TraceError::BadRequest(msg.into())
}

pub fn unauthorized() -> TraceError {
    TraceError::Unauthorized("Unauthorized".to_string())
}

pub fn unauthorized_msg(msg: impl Into<String>) -> TraceError {
    TraceError::Unauthorized(msg.into())
}

pub fn forbidden() -> TraceError {
    TraceError::Forbidden("Forbidden".to_string())
}

pub fn forbidden_msg(msg: impl Into<String>) -> TraceError {
    TraceError::Forbidden(msg.into())
}

pub fn not_found() -> TraceError {
    TraceError::NotFound("Not found".to_string())
}

pub fn not_found_msg(msg: impl Into<String>) -> TraceError {
    TraceError::NotFound(msg.into())
}

pub fn conflict() -> TraceError {
    TraceError::Conflict("Conflict".to_string())
}

pub fn conflict_msg(msg: impl Into<String>) -> TraceError {
    TraceError::Conflict(msg.into())
}

pub fn internal_error() -> TraceError {
    TraceError::InternalError("Internal server error".to_string())
}

pub fn internal_error_msg(msg: impl Into<String>) -> TraceError {
    TraceError::InternalError(msg.into())
}

impl From<JsonRejection> for TraceError {
    fn from(rejection: JsonRejection) -> Self {
        let message = match rejection {
            JsonRejection::JsonDataError(err) => {
                format!("Invalid JSON data: {}", err)
            }
            JsonRejection::JsonSyntaxError(err) => {
                format!("Invalid JSON syntax: {}", err)
            }
            JsonRejection::MissingJsonContentType(_) => {
                "Missing Content-Type header (application/json)".to_string()
            }
            JsonRejection::BytesRejection(err) => {
                format!("Failed to read request body: {}", err)
            }
            _ => "Invalid request body".to_string(),
        };

        TraceError::BadRequest(message)
    }
}
