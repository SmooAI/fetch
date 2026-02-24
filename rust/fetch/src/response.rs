//! Response types for the smooai-fetch client.

use std::collections::HashMap;

/// A generic response structure that holds parsed data and metadata.
///
/// Analogous to the TypeScript `ResponseWithBody<T>`.
#[derive(Debug, Clone)]
pub struct FetchResponse<T> {
    /// HTTP status code.
    pub status: u16,
    /// HTTP status text (reason phrase).
    pub status_text: String,
    /// Whether the response was successful (status 200-299).
    pub ok: bool,
    /// Response headers.
    pub headers: HashMap<String, String>,
    /// Whether the response body was parsed as JSON.
    pub is_json: bool,
    /// The raw response body as a string.
    pub body: String,
    /// The parsed/deserialized response data, if available.
    pub data: Option<T>,
}

impl<T> FetchResponse<T> {
    /// Create a new FetchResponse from raw components.
    pub fn new(
        status: u16,
        status_text: String,
        headers: HashMap<String, String>,
        body: String,
        is_json: bool,
        data: Option<T>,
    ) -> Self {
        let ok = (200..300).contains(&(status as u32));
        Self {
            status,
            status_text,
            ok,
            headers,
            is_json,
            body,
            data,
        }
    }
}
