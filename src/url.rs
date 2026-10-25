//! URL validation and query encoding for the ListenBrainz API.
//!
//! Everything here is pure: it never touches the network and never depends on
//! the plugin bindings, so it is unit-tested on the host target.

/// Base URL of the ListenBrainz web API.
pub const DEFAULT_BASE_URL: &str = "https://api.listenbrainz.org";

/// Base URL of the ListenBrainz labs API, which serves the similarity datasets.
///
/// The `labsBaseUrl` setting defaults to this value and can point the
/// similarity calls at a labs mirror or a test double.
pub const DEFAULT_LABS_BASE_URL: &str = "https://labs.api.listenbrainz.org";

/// Sessions-based algorithm used for artist similarity.
///
/// The labs API takes the algorithm as a string naming a precomputed dataset,
/// so the name has to be spelled out in full.
pub const SIMILAR_ARTISTS_ALGORITHM: &str =
    "session_based_days_7500_session_300_contribution_5_threshold_10_limit_100_filter_True_skip_30";

/// Sessions-based algorithm used for recording similarity.
pub const SIMILAR_RECORDINGS_ALGORITHM: &str =
    "session_based_days_180_session_300_contribution_5_threshold_15_limit_50_skip_30";

/// Returns whether `value` is an absolute `http` or `https` URL.
///
/// The check is deliberately structural rather than a full URL parse: it
/// accepts exactly the shapes a ListenBrainz base URL can have (the public API,
/// a mirror, or a test double on a private host), and rejects everything a
/// request could not be built from.
///
/// # Parameters
///
/// - `value`: the candidate URL.
///
/// # Returns
///
/// `true` when `value` starts with `http://` or `https://`, carries a non-empty
/// origin, and contains no whitespace.
///
/// # Example
///
/// ```text
/// is_http_url("https://api.listenbrainz.org") -> true
/// is_http_url("http://localhost:8080")        -> true
/// is_http_url("api.listenbrainz.org")         -> false
/// is_http_url("ftp://api.listenbrainz.org")   -> false
/// ```
pub fn is_http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("https://")
        .or_else(|| value.strip_prefix("http://"))
    else {
        return false;
    };

    if value.chars().any(char::is_whitespace) {
        return false;
    }

    let authority = rest.split('/').next().unwrap_or("");
    !authority.is_empty()
}

/// Normalizes a configured base URL.
///
/// Surrounding whitespace and trailing slashes are removed so that the
/// configured value and the URLs built from it do not double up on `/`.
///
/// # Parameters
///
/// - `value`: the configured base URL.
///
/// # Returns
///
/// The trimmed URL, without a trailing slash.
///
/// # Example
///
/// ```text
/// normalize_base_url(" https://api.listenbrainz.org/ ") -> "https://api.listenbrainz.org"
/// normalize_base_url("https://api.listenbrainz.org")   -> "https://api.listenbrainz.org"
/// ```
pub fn normalize_base_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

/// Joins a base URL with an API path.
///
/// # Parameters
///
/// - `base_url`: the configured base URL, with or without a trailing slash.
/// - `path`: the API path, with or without a leading slash.
///
/// # Returns
///
/// The absolute URL of the endpoint.
///
/// # Example
///
/// ```text
/// endpoint("https://api.listenbrainz.org/", "/1/validate-token")
///     -> "https://api.listenbrainz.org/1/validate-token"
/// endpoint("https://api.listenbrainz.org", "1/submit-listens")
///     -> "https://api.listenbrainz.org/1/submit-listens"
/// ```
pub fn endpoint(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

/// Percent-encodes one query or path component.
///
/// Everything outside the RFC 3986 unreserved set is escaped, including `/`,
/// `&`, `=`, `?` and `#`, so a user name or an identifier cannot add parameters
/// or change the endpoint it is pasted into. A space becomes `%20` rather than
/// `+`, which every ListenBrainz endpoint accepts.
///
/// # Parameters
///
/// - `value`: the component to encode.
///
/// # Returns
///
/// The encoded component.
///
/// # Example
///
/// ```text
/// encode_component("rob")          -> "rob"
/// encode_component("a b&c")        -> "a%20b%26c"
/// encode_component("Bjork / Joga") -> "Bjork%20%2F%20Joga"
/// ```
pub fn encode_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(char::from(*byte));
            }
            _ => {
                encoded.push('%');
                encoded.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
                encoded.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
            }
        }
    }
    encoded
}

/// Uppercase hexadecimal digits used by [`encode_component`].
const HEX_DIGITS: [u8; 16] = *b"0123456789ABCDEF";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https_urls() {
        assert!(is_http_url("https://api.listenbrainz.org"));
        assert!(is_http_url("http://localhost:8080"));
        assert!(is_http_url("https://mirror.example.org/listenbrainz/"));
    }

    #[test]
    fn rejects_urls_without_a_host_or_scheme() {
        assert!(!is_http_url("api.listenbrainz.org"));
        assert!(!is_http_url("ftp://api.listenbrainz.org"));
        assert!(!is_http_url("https://"));
        assert!(!is_http_url("https:///1/validate-token"));
        assert!(!is_http_url("http://api listenbrainz.org"));
    }

    #[test]
    fn normalization_removes_surrounding_space_and_trailing_slashes() {
        assert_eq!(
            normalize_base_url("  https://x.example/  "),
            "https://x.example"
        );
        assert_eq!(normalize_base_url("https://x.example"), "https://x.example");
    }

    #[test]
    fn endpoint_joins_exactly_one_slash() {
        assert_eq!(
            endpoint("https://api.listenbrainz.org", "1/validate-token"),
            "https://api.listenbrainz.org/1/validate-token"
        );
        assert_eq!(
            endpoint("https://api.listenbrainz.org/", "/1/submit-listens"),
            "https://api.listenbrainz.org/1/submit-listens"
        );
    }

    #[test]
    fn encoding_escapes_reserved_characters() {
        assert_eq!(encode_component("rob"), "rob");
        assert_eq!(encode_component("a b&c"), "a%20b%26c");
        assert_eq!(encode_component("a=1&b=2"), "a%3D1%26b%3D2");
        assert_eq!(encode_component("Bjork / Joga"), "Bjork%20%2F%20Joga");
    }

    #[test]
    fn encoding_escapes_every_utf8_byte() {
        assert_eq!(encode_component("Royksopp - Joga"), "Royksopp%20-%20Joga");
        assert_eq!(
            encode_component("Sigur Ros: ( )"),
            "Sigur%20Ros%3A%20%28%20%29"
        );
    }
}
