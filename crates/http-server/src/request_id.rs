use axum::http::{HeaderName, HeaderValue, Request};
use tower_http::request_id::{MakeRequestId, RequestId};
use uuid::Uuid;

/// The standard header name for request correlation IDs (`x-request-id`).
pub static REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Generator for UUID v4 request IDs.
#[derive(Clone, Copy, Debug, Default)]
pub struct MakeRequestUuid;

impl MakeRequestId for MakeRequestUuid {
    fn make_request_id<B>(&mut self, _request: &Request<B>) -> Option<RequestId> {
        let id = Uuid::new_v4().to_string();
        HeaderValue::from_str(&id).ok().map(RequestId::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::unwrap_used)]
    fn generates_valid_request_id() {
        let mut maker = MakeRequestUuid;
        let req = Request::builder().body(()).unwrap();
        let id = maker.make_request_id(&req);
        assert!(id.is_some());
    }
}
