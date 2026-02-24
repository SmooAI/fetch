//! Error types for the smooai-fetch client.

use crate::response::FetchResponse;

/// All possible errors from the fetch client.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// HTTP response error (non-2xx status code).
    #[error("{message}; HTTP Error Response: {status} {status_text}")]
    HttpResponse {
        /// HTTP status code.
        status: u16,
        /// HTTP status text.
        status_text: String,
        /// Extracted error message.
        message: String,
        /// Response headers.
        headers: std::collections::HashMap<String, String>,
        /// Raw response body as string.
        body: String,
        /// Whether response was JSON.
        is_json: bool,
    },

    /// Retry attempts exhausted.
    #[error("Retry Error: Ran out of retry attempts after {attempts} retries; {source}")]
    Retry {
        /// Number of attempts made.
        attempts: u32,
        /// The last error that caused the final retry to fail.
        #[source]
        source: Box<FetchError>,
    },

    /// Rate limit exceeded.
    #[error("Rate limit exceeded: {remaining_ms}ms remaining in rate limit window")]
    RateLimit {
        /// Milliseconds remaining in the rate limit window.
        remaining_ms: u64,
    },

    /// Circuit breaker is open, rejecting requests.
    #[error("Circuit breaker is open: requests are being rejected")]
    CircuitBreaker,

    /// Request timed out.
    #[error("Request timed out after {timeout_ms}ms")]
    Timeout {
        /// The timeout duration in milliseconds.
        timeout_ms: u64,
    },

    /// Schema/deserialization validation failed.
    #[error("Schema validation error: {message}")]
    SchemaValidation {
        /// The validation error message.
        message: String,
    },

    /// Underlying request error from reqwest.
    #[error("Request error: {0}")]
    Request(#[from] reqwest::Error),
}

impl FetchError {
    /// Returns true if this error represents a retryable condition.
    /// Status 429 (rate limited) and 5xx (server error) are retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            FetchError::HttpResponse { status, .. } => *status == 429 || *status >= 500,
            FetchError::Timeout { .. } => true,
            FetchError::Request(_) => true,
            _ => false,
        }
    }

    /// Extract the Retry-After header value in seconds, if present.
    pub fn retry_after_secs(&self) -> Option<u64> {
        match self {
            FetchError::HttpResponse { headers, .. } => {
                headers.get("retry-after").and_then(|v| v.parse::<u64>().ok())
            }
            _ => None,
        }
    }

    /// Create an HttpResponse error from a FetchResponse.
    pub fn from_response<T>(response: &FetchResponse<T>, msg: Option<&str>) -> Self {
        let error_str = extract_error_string(&response.body, response.is_json);
        let message = match msg {
            Some(m) => format!("{}; {}", m, error_str),
            None => error_str,
        };
        FetchError::HttpResponse {
            status: response.status,
            status_text: response.status_text.clone(),
            message,
            headers: response.headers.clone(),
            body: response.body.clone(),
            is_json: response.is_json,
        }
    }
}

/// Extract a human-readable error string from a response body.
fn extract_error_string(body: &str, is_json: bool) -> String {
    if !is_json {
        if body.is_empty() {
            return "Unknown error".to_string();
        }
        return body.to_string();
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        let mut error_str = String::new();
        let mut err_is_set = false;

        if let Some(error) = value.get("error") {
            if !error.is_array() {
                if let Some(error_type) = error.get("type").and_then(|v| v.as_str()) {
                    error_str.push_str(&format!("({}): ", error_type));
                    err_is_set = true;
                }
                if let Some(code) = error.get("code") {
                    let code_str = if let Some(n) = code.as_u64() {
                        n.to_string()
                    } else if let Some(s) = code.as_str() {
                        s.to_string()
                    } else {
                        code.to_string()
                    };
                    error_str.push_str(&format!("({}): ", code_str));
                    err_is_set = true;
                }
                if let Some(message) = error.get("message").and_then(|v| v.as_str()) {
                    error_str.push_str(message);
                    err_is_set = true;
                }
                if let Some(error_text) = error.as_str() {
                    error_str.push_str(error_text);
                    err_is_set = true;
                }
            }
        }

        if let Some(error_messages) = value.get("errorMessages").and_then(|v| v.as_array()) {
            let msgs: Vec<String> = error_messages
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if !msgs.is_empty() {
                error_str.push_str(&msgs.join("; "));
                err_is_set = true;
            }
        }

        if !err_is_set {
            if body.is_empty() {
                return "Unknown error".to_string();
            }
            return body.to_string();
        }

        return error_str;
    }

    if body.is_empty() {
        "Unknown error".to_string()
    } else {
        body.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_error_string_with_error_object() {
        let body =
            r#"{"error":{"type":"ERROR_TYPE","code":125,"message":"Error message"}}"#;
        let result = extract_error_string(body, true);
        assert!(result.contains("ERROR_TYPE"));
        assert!(result.contains("125"));
        assert!(result.contains("Error message"));
    }

    #[test]
    fn test_extract_error_string_with_error_string() {
        let body = r#"{"error":"Simple error"}"#;
        let result = extract_error_string(body, true);
        assert!(result.contains("Simple error"));
    }

    #[test]
    fn test_extract_error_string_with_error_messages() {
        let body = r#"{"errorMessages":["Error 1","Error 2"]}"#;
        let result = extract_error_string(body, true);
        assert!(result.contains("Error 1"));
        assert!(result.contains("Error 2"));
    }

    #[test]
    fn test_extract_error_string_non_json() {
        let result = extract_error_string("Plain text error", false);
        assert_eq!(result, "Plain text error");
    }

    #[test]
    fn test_extract_error_string_empty() {
        let result = extract_error_string("", false);
        assert_eq!(result, "Unknown error");
    }

    #[test]
    fn test_is_retryable() {
        let err429 = FetchError::HttpResponse {
            status: 429,
            status_text: "Too Many Requests".to_string(),
            message: "".to_string(),
            headers: std::collections::HashMap::new(),
            body: "".to_string(),
            is_json: false,
        };
        assert!(err429.is_retryable());

        let err500 = FetchError::HttpResponse {
            status: 500,
            status_text: "Internal Server Error".to_string(),
            message: "".to_string(),
            headers: std::collections::HashMap::new(),
            body: "".to_string(),
            is_json: false,
        };
        assert!(err500.is_retryable());

        let err404 = FetchError::HttpResponse {
            status: 404,
            status_text: "Not Found".to_string(),
            message: "".to_string(),
            headers: std::collections::HashMap::new(),
            body: "".to_string(),
            is_json: false,
        };
        assert!(!err404.is_retryable());

        let timeout_err = FetchError::Timeout { timeout_ms: 5000 };
        assert!(timeout_err.is_retryable());
    }

    #[test]
    fn test_retry_after_secs() {
        let mut headers = std::collections::HashMap::new();
        headers.insert("retry-after".to_string(), "5".to_string());

        let err = FetchError::HttpResponse {
            status: 429,
            status_text: "Too Many Requests".to_string(),
            message: "".to_string(),
            headers,
            body: "".to_string(),
            is_json: false,
        };
        assert_eq!(err.retry_after_secs(), Some(5));
    }
}
