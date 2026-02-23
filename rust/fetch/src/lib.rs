//! SmooAI Fetch Client for Rust.
//!
//! A resilient HTTP fetch client with retries, timeouts, rate limiting,
//! and circuit breaking.

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert_eq!(VERSION, "2.1.2");
    }
}
