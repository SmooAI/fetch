//! Credential redaction for anything this client hands to a logger.
//!
//! SMOODEV-2716. The Rust client's only logging sink for request data is the
//! `url` field on the `Sending HTTP request` tracing event, so that is what is
//! scrubbed here — a URL carries credentials in two places, the userinfo
//! password and the query string. Bodies and headers are never logged by this
//! crate, so there is deliberately nothing here for them; if a sink is ever
//! added, extend this module and load the matching group from
//! `spec/redaction-corpus.json` in the same change.
//!
//! This is log-only. The request actually sent on the wire is never touched.

pub(crate) const REDACTED: &str = "[REDACTED]";

/// A key is sensitive when its normalized form contains one of these.
/// Mirrors `sensitiveKeys.contains` in `spec/redaction-corpus.json`.
const SENSITIVE_PARTS: &[&str] = &[
    "secret",
    "password",
    "passwd",
    "token",
    "apikey",
    "authorization",
    "credential",
    "privatekey",
    "assertion",
    "cookie",
    "session",
    "signature",
];

/// ...or equals one of these. Mirrors `sensitiveKeys.equals`. Too short or too
/// common to substring-match without shredding unrelated fields.
const SENSITIVE_EXACT: &[&str] = &["auth", "code", "pwd", "sig"];

/// `X-Api-Key`, `x_api_key` and `apiKey` are the same key as far as a leak is
/// concerned, so separators and case are removed before matching.
fn normalize_key(key: &str) -> String {
    key.chars()
        .filter(|c| !matches!(c, '-' | '_' | '.'))
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) fn is_sensitive_key(key: &str) -> bool {
    let k = normalize_key(key);
    SENSITIVE_EXACT.contains(&k.as_str()) || SENSITIVE_PARTS.iter().any(|part| k.contains(part))
}

/// Redact sensitive params from `a=1&b=2`. Splits each pair on the FIRST `=` so
/// a base64 value keeps its `=` padding.
fn redact_pairs(pairs: &str) -> String {
    pairs
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((key, _)) if is_sensitive_key(key) => format!("{key}={REDACTED}"),
            _ => pair.to_string(),
        })
        .collect::<Vec<_>>()
        .join("&")
}

/// Redact sensitive params from a query string (leading `?` optional).
pub(crate) fn redact_query_string(search: &str) -> String {
    match search.strip_prefix('?') {
        Some(rest) => format!("?{}", redact_pairs(rest)),
        None if search.is_empty() => String::new(),
        None => redact_pairs(search),
    }
}

/// Redact credentials from a URL string: the userinfo password and the query
/// params. Purely textual, so relative and non-parseable URLs work unchanged
/// and nothing is normalized behind the caller's back.
pub(crate) fn redact_url(url: &str) -> String {
    let (base, fragment) = match url.find('#') {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, ""),
    };

    let with_query = match base.find('?') {
        Some(i) => format!("{}{}", &base[..i], redact_query_string(&base[i..])),
        None => base.to_string(),
    };

    format!("{}{fragment}", redact_userinfo_password(&with_query))
}

/// `scheme://user:password@host` -> `scheme://user:[REDACTED]@host`.
fn redact_userinfo_password(url: &str) -> String {
    // Authority runs from after `://` to the first `/`, `?` or `#`.
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let authority_start = scheme_end + 3;
    let authority_end = url[authority_start..]
        .find(['/', '?', '#'])
        .map_or(url.len(), |i| authority_start + i);
    let authority = &url[authority_start..authority_end];

    let Some(at) = authority.rfind('@') else {
        return url.to_string();
    };
    let Some((user, _)) = authority[..at].split_once(':') else {
        return url.to_string();
    };

    format!(
        "{}{user}:{REDACTED}@{}",
        &url[..authority_start],
        &url[authority_start + at + 1..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Shared with the TypeScript suite. `include_str!` binds it at compile time,
    // so the corpus cannot silently go stale relative to this test.
    const CORPUS: &str = include_str!("../../../spec/redaction-corpus.json");

    fn corpus() -> serde_json::Value {
        serde_json::from_str(CORPUS).expect("redaction corpus must parse")
    }

    fn cases<'a>(corpus: &'a serde_json::Value, group: &str) -> &'a Vec<serde_json::Value> {
        corpus["cases"][group]
            .as_array()
            .unwrap_or_else(|| panic!("corpus group `{group}` missing"))
    }

    // Positive control: a corpus that failed to load would otherwise read as
    // "all tests passed".
    #[test]
    fn corpus_loads() {
        let c = corpus();
        assert_eq!(c["redactedWith"], REDACTED);
        for group in ["url", "query"] {
            assert!(!cases(&c, group).is_empty(), "group `{group}` is empty");
        }
    }

    #[test]
    fn url_cases() {
        let c = corpus();
        for case in cases(&c, "url") {
            let input = case["input"].as_str().unwrap();
            let expected = case["expected"].as_str().unwrap();
            assert_eq!(
                redact_url(input),
                expected,
                "url case `{}`",
                case["name"].as_str().unwrap_or("?")
            );
        }
    }

    #[test]
    fn query_cases() {
        let c = corpus();
        for case in cases(&c, "query") {
            let input = case["input"].as_str().unwrap();
            let expected = case["expected"].as_str().unwrap();
            assert_eq!(
                redact_query_string(input),
                expected,
                "query case `{}`",
                case["name"].as_str().unwrap_or("?")
            );
        }
    }

    // The key list is the security contract; both languages match on the same
    // roots, so it lives in the corpus rather than in either implementation.
    #[test]
    fn sensitive_key_roots_match_the_corpus() {
        let c = corpus();
        for part in c["sensitiveKeys"]["contains"].as_array().unwrap() {
            let part = part.as_str().unwrap();
            assert!(
                is_sensitive_key(&format!("x_{part}_value")),
                "expected a key containing `{part}` to be sensitive"
            );
        }
        for key in c["sensitiveKeys"]["equals"].as_array().unwrap() {
            let key = key.as_str().unwrap();
            assert!(is_sensitive_key(key), "expected `{key}` to be sensitive");
        }
    }

    #[test]
    fn safe_identifiers_are_not_redacted() {
        for safe in [
            "country_code",
            "zipcode",
            "user_id",
            "client_id",
            "grant_type",
            "scope",
            "state",
        ] {
            assert!(!is_sensitive_key(safe), "`{safe}` should not be redacted");
        }
    }
}
