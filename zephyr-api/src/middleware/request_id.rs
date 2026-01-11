//! Request ID middleware.

use axum::{
    body::Body,
    http::{Request, header::HeaderName},
    response::Response,
};
use std::task::{Context, Poll};
use tower::{Layer, Service};
use uuid::Uuid;

/// Request ID header name.
pub static REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Request ID extracted from or generated for a request.
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

impl RequestId {
    /// Generates a new request ID.
    #[must_use]
    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Creates a request ID from a string.
    #[must_use]
    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the request ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Layer for adding request IDs.
#[derive(Debug, Clone, Default)]
pub struct RequestIdLayer;

impl RequestIdLayer {
    /// Creates a new request ID layer.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl<S> Layer<S> for RequestIdLayer {
    type Service = RequestIdService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestIdService { inner }
    }
}

/// Service that adds request IDs.
#[derive(Debug, Clone)]
pub struct RequestIdService<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for RequestIdService<S>
where
    S: Service<Request<Body>, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        // Extract or generate request ID
        let request_id = request
            .headers()
            .get(&REQUEST_ID_HEADER)
            .and_then(|h| h.to_str().ok())
            .map_or_else(RequestId::generate, |s| {
                RequestId::from_string(s.to_string())
            });

        // Store in extensions
        request.extensions_mut().insert(request_id.clone());

        let mut inner = self.inner.clone();

        Box::pin(async move {
            let mut response = inner.call(request).await?;

            // Add request ID to response headers
            response
                .headers_mut()
                .insert(&REQUEST_ID_HEADER, request_id.as_str().parse().unwrap());

            Ok(response)
        })
    }
}

/// Extractor for request ID.
impl<S> axum::extract::FromRequestParts<S> for RequestId
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(parts
            .extensions
            .get::<RequestId>()
            .cloned()
            .unwrap_or_else(RequestId::generate))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_id_generate() {
        let id1 = RequestId::generate();
        let id2 = RequestId::generate();

        // Should generate unique IDs
        assert_ne!(id1.as_str(), id2.as_str());

        // Should be valid UUIDs
        assert!(Uuid::parse_str(id1.as_str()).is_ok());
        assert!(Uuid::parse_str(id2.as_str()).is_ok());
    }

    #[test]
    fn test_request_id_from_string() {
        let id = RequestId::from_string("custom-id-123");
        assert_eq!(id.as_str(), "custom-id-123");
    }

    #[test]
    fn test_request_id_display() {
        let id = RequestId::from_string("test-id");
        assert_eq!(format!("{id}"), "test-id");
    }
}
