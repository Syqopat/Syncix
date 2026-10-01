use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

/// Every failure the HTTP surface can answer with.
///
/// Handlers used to pick their own status code, and some answered `200 OK` with an
/// `{"error": ...}` body, so a client could not tell success from failure by status
/// alone. One type, one mapping, one body shape.
#[derive(Debug, Clone)]
pub enum ApiError {
    BadRequest(String),
    Unauthorized(String),
    NotFound(String),
    Conflict(String),
    Internal(String),
}

impl ApiError {
    fn parts(&self) -> (StatusCode, &'static str, &str) {
        match self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, "bad_request", m.as_str()),
            ApiError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, "unauthorized", m.as_str()),
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, "not_found", m.as_str()),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, "conflict", m.as_str()),
            ApiError::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, "internal", m.as_str()),
        }
    }

    pub fn status(&self) -> StatusCode {
        self.parts().0
    }

    pub fn message(&self) -> String {
        self.parts().2.to_string()
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, kind, message) = self.parts();
        (
            status,
            Json(serde_json::json!({ "error": kind, "message": message })),
        )
            .into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;

/// The reply of a call that only reports that it was accepted.
pub fn accepted() -> Response {
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "status": "accepted" })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_failure_has_its_own_status() {
        assert_eq!(ApiError::BadRequest("x".into()).status(), StatusCode::BAD_REQUEST);
        assert_eq!(ApiError::Unauthorized("x".into()).status(), StatusCode::UNAUTHORIZED);
        assert_eq!(ApiError::NotFound("x".into()).status(), StatusCode::NOT_FOUND);
        assert_eq!(ApiError::Conflict("x".into()).status(), StatusCode::CONFLICT);
        assert_eq!(
            ApiError::Internal("x".into()).status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
