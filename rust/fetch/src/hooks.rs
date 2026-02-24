//! Lifecycle hooks for request/response handling.

use crate::error::FetchError;
use crate::response::FetchResponse;
use crate::types::RequestInit;

/// Hook that runs before the request is made, allowing modification of the URL and init.
///
/// Returns `Some((modified_url, modified_init))` to modify the request,
/// or `None` to leave it unchanged.
pub type PreRequestHook =
    Box<dyn Fn(&str, &RequestInit) -> Option<(String, RequestInit)> + Send + Sync>;

/// Hook that runs after a successful response, allowing modification of the response.
///
/// Returns `Some(modified_response)` to replace the response, or `None` to
/// leave it unchanged.
pub type PostResponseSuccessHook<T> =
    Box<dyn Fn(&str, &RequestInit, FetchResponse<T>) -> Option<FetchResponse<T>> + Send + Sync>;

/// Hook that runs after a failed response, allowing modification or replacement of the error.
///
/// Returns `Some(modified_error)` to replace the error, or `None` to leave
/// the original error unchanged.
pub type PostResponseErrorHook<T> = Box<
    dyn Fn(&str, &RequestInit, &FetchError, Option<&FetchResponse<T>>) -> Option<FetchError>
        + Send
        + Sync,
>;

/// Collection of lifecycle hooks for request/response handling.
pub struct LifecycleHooks<T> {
    /// Hook that runs before the request is made.
    pub pre_request: Option<PreRequestHook>,
    /// Hook that runs after a successful response.
    pub post_response_success: Option<PostResponseSuccessHook<T>>,
    /// Hook that runs after a failed response.
    pub post_response_error: Option<PostResponseErrorHook<T>>,
}

impl<T> Default for LifecycleHooks<T> {
    fn default() -> Self {
        Self {
            pre_request: None,
            post_response_success: None,
            post_response_error: None,
        }
    }
}

impl<T> std::fmt::Debug for LifecycleHooks<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LifecycleHooks")
            .field("pre_request", &self.pre_request.is_some())
            .field("post_response_success", &self.post_response_success.is_some())
            .field("post_response_error", &self.post_response_error.is_some())
            .finish()
    }
}
