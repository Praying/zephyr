//! JWT authentication middleware.

use axum::{
    Json,
    body::Body,
    extract::State,
    http::{Request, StatusCode, header::AUTHORIZATION},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

use crate::auth::{Claims, JwtManager, UserRole, extract_bearer_token};
use crate::error::ErrorResponse;
use crate::state::AppState;

/// Authenticated user information extracted from JWT.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    /// User ID
    pub user_id: String,
    /// User role
    pub role: UserRole,
    /// Tenant ID (for multi-tenant support)
    pub tenant_id: Option<String>,
    /// Full claims
    pub claims: Claims,
}

impl AuthenticatedUser {
    /// Creates an authenticated user from claims.
    #[must_use]
    pub fn from_claims(claims: Claims) -> Self {
        Self {
            user_id: claims.sub.clone(),
            role: claims.role,
            tenant_id: claims.tenant_id.clone(),
            claims,
        }
    }
}

/// Authentication layer for protecting routes.
#[derive(Clone)]
pub struct AuthLayer {
    jwt_manager: Arc<JwtManager>,
}

impl AuthLayer {
    /// Creates a new authentication layer.
    #[must_use]
    pub fn new(jwt_manager: Arc<JwtManager>) -> Self {
        Self { jwt_manager }
    }
}

/// Authentication middleware function.
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    // Extract Authorization header
    let auth_header = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let Some(auth_header) = auth_header else {
        return unauthorized_response("Missing Authorization header");
    };

    // Extract bearer token
    let Some(token) = extract_bearer_token(auth_header) else {
        return unauthorized_response("Invalid Authorization header format");
    };

    // Validate token
    match state.jwt_manager.validate_token(token) {
        Ok(claims) => {
            // Store authenticated user in request extensions
            let user = AuthenticatedUser::from_claims(claims);
            request.extensions_mut().insert(user);
            next.run(request).await
        }
        Err(e) => unauthorized_response(&e.to_string()),
    }
}

/// Creates an unauthorized response.
fn unauthorized_response(message: &str) -> Response {
    let body = ErrorResponse {
        status: "error",
        code: "UNAUTHORIZED",
        message: message.to_string(),
        request_id: None,
    };

    (StatusCode::UNAUTHORIZED, Json(body)).into_response()
}

/// Role-based authorization middleware.
pub async fn require_role(
    required_roles: &[UserRole],
    request: &Request<Body>,
) -> Result<(), Response> {
    let user = request
        .extensions()
        .get::<AuthenticatedUser>()
        .ok_or_else(|| unauthorized_response("Not authenticated"))?;

    if required_roles.contains(&user.role) {
        Ok(())
    } else {
        Err(forbidden_response("Insufficient permissions"))
    }
}

/// Creates a forbidden response.
fn forbidden_response(message: &str) -> Response {
    let body = ErrorResponse {
        status: "error",
        code: "FORBIDDEN",
        message: message.to_string(),
        request_id: None,
    };

    (StatusCode::FORBIDDEN, Json(body)).into_response()
}

/// Extractor for authenticated user.
#[derive(Debug, Clone)]
pub struct Auth(pub AuthenticatedUser);

impl<S> axum::extract::FromRequestParts<S> for Auth
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthenticatedUser>()
            .cloned()
            .map(Auth)
            .ok_or_else(|| unauthorized_response("Not authenticated"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authenticated_user_from_claims() {
        let claims = Claims {
            sub: "user123".to_string(),
            iss: "test".to_string(),
            aud: "test".to_string(),
            exp: 0,
            iat: 0,
            nbf: 0,
            jti: "jti".to_string(),
            role: UserRole::Trader,
            tenant_id: Some("tenant1".to_string()),
        };

        let user = AuthenticatedUser::from_claims(claims);
        assert_eq!(user.user_id, "user123");
        assert_eq!(user.role, UserRole::Trader);
        assert_eq!(user.tenant_id, Some("tenant1".to_string()));
    }
}
