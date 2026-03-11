use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::{Next},
    response::{IntoResponse, Response},
    Json,
};
use std::sync::Arc;

use crate::config::Config;
use crate::app::models::ErrorResponse;

/// Authentication state
#[derive(Clone)]
pub struct AuthState {
    config: Arc<Config>,
}

impl AuthState {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    /// Check if a token is valid
    pub fn is_valid_token(&self, token: &str) -> bool {
        self.config.is_valid_token(token)
    }

    /// Check if authentication is required
    pub fn is_auth_enabled(&self) -> bool {
        self.config.is_auth_enabled()
    }
}

/// Extract Bearer token from Authorization header
pub fn extract_bearer_token(auth_header: Option<&str>) -> Option<&str> {
    auth_header
        .and_then(|h| h.strip_prefix("Bearer "))
        .filter(|t| !t.is_empty())
}

/// Authentication middleware
pub async fn auth_middleware(
    auth_state: AuthState,
    request: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    // If auth is not enabled, allow all requests
    if !auth_state.is_auth_enabled() {
        return Ok(next.run(request).await);
    }

    // Extract token from Authorization header
    let token = extract_bearer_token(
        request
            .headers()
            .get("Authorization")
            .and_then(|v| v.to_str().ok()),
    );

    match token {
        Some(t) if auth_state.is_valid_token(t) => Ok(next.run(request).await),
        _ => {
            let mut response = (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse::unauthorized("Invalid or missing authentication token")),
            )
                .into_response();
            response.headers_mut().insert(
                "WWW-Authenticate",
                "Bearer".parse().expect("valid header value"),
            );
            Err(response)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_bearer_token_valid() {
        let token = extract_bearer_token(Some("Bearer abc123"));
        assert_eq!(token, Some("abc123"));
    }

    #[test]
    fn extract_bearer_token_missing_prefix() {
        let token = extract_bearer_token(Some("abc123"));
        assert_eq!(token, None);
    }

    #[test]
    fn extract_bearer_token_empty() {
        let token = extract_bearer_token(Some("Bearer "));
        assert_eq!(token, None);
    }

    #[test]
    fn extract_bearer_token_none() {
        let token = extract_bearer_token(None);
        assert_eq!(token, None);
    }
}
