//! HTTP access to the ListenBrainz API through the `host-http` host import.
//!
//! This is the only module that touches the network. It builds one request,
//! sends it, and either decodes the JSON body or maps the response to a
//! provider error; endpoint composition lives in [`crate::scrobble`],
//! [`crate::discovery`] and [`crate::link`].
//!
//! ListenBrainz reports an invalid token with HTTP 401 and a rate limit with
//! HTTP 429, so the SDK's [`status_error`](hearthsound_plugin::http::status_error)
//! mapping applies unchanged.

use hearthsound_plugin::host::http;
use hearthsound_plugin::types::ProviderError;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// Wall-clock budget for one ListenBrainz request, in milliseconds.
///
/// The host clamps this to its own ceiling and to the remaining call deadline.
const REQUEST_TIMEOUT_MS: u32 = 10_000;

/// Most recording mbids one metadata lookup may carry.
///
/// The endpoint publishes a cap for the bulk form; staying below it keeps a
/// large recommendation list in a handful of requests.
pub(crate) const METADATA_BATCH: usize = 50;

/// One authenticated (or anonymous) ListenBrainz client.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Client<'a> {
    /// Base URL of the API, without a trailing slash.
    pub(crate) base_url: &'a str,
    /// User token sent in the `authorization` header, when one is known.
    pub(crate) token: Option<&'a str>,
}

impl Client<'_> {
    /// Sends a GET request and checks the status.
    ///
    /// # Parameters
    ///
    /// - `path`: API path, without a leading slash.
    ///
    /// # Returns
    ///
    /// `Ok(())` for a `2xx` response.
    ///
    /// # Errors
    ///
    /// Returns the mapped response error, and [`ProviderError::Unavailable`]
    /// when the host refuses or cannot complete the request.
    pub(crate) fn check(&self, path: &str) -> Result<(), ProviderError> {
        let response = self.send("GET", path, None)?;
        if hearthsound_plugin::http::is_success(&response) {
            Ok(())
        } else {
            Err(hearthsound_plugin::http::status_error(&response))
        }
    }

    /// Sends a GET request and decodes the JSON body.
    ///
    /// # Parameters
    ///
    /// - `path`: API path, without a leading slash.
    ///
    /// # Returns
    ///
    /// The decoded response body.
    ///
    /// # Errors
    ///
    /// Returns the mapped response error, [`ProviderError::Unavailable`] when
    /// the host refuses or cannot complete the request, and
    /// [`ProviderError::Internal`] when the body is not JSON of the expected
    /// shape.
    pub(crate) fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, ProviderError> {
        let response = self.send("GET", path, None)?;
        decode(response)
    }

    /// Sends a GET request and decodes the JSON body unless the endpoint
    /// answered `204 No Content`.
    ///
    /// # Parameters
    ///
    /// - `path`: API path, without a leading slash.
    ///
    /// # Returns
    ///
    /// `None` when ListenBrainz answered `204`, and the decoded body otherwise.
    ///
    /// # Errors
    ///
    /// Returns the mapped response error, and [`ProviderError::Internal`] when
    /// a `2xx` body is not JSON of the expected shape.
    pub(crate) fn get_optional<T: DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<Option<T>, ProviderError> {
        let response = self.send("GET", path, None)?;
        decode_optional(response)
    }

    /// Sends a POST request with a JSON body and decodes the JSON answer.
    ///
    /// # Parameters
    ///
    /// - `path`: API path, without a leading slash.
    /// - `body`: value serialized to JSON as the request body.
    ///
    /// # Returns
    ///
    /// The decoded response body.
    ///
    /// # Errors
    ///
    /// Returns the mapped response error and [`ProviderError::Internal`] when
    /// the body cannot be serialized or the answer is not JSON of the expected
    /// shape.
    pub(crate) fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, ProviderError> {
        let body = serde_json::to_vec(body)
            .map_err(|error| ProviderError::Internal(format!("request body: {error}")))?;
        let response = self.send("POST", path, Some(body))?;
        decode(response)
    }

    /// Sends a request whose success carries no body the plugin reads.
    ///
    /// # Parameters
    ///
    /// - `method`: HTTP method.
    /// - `path`: API path, without a leading slash.
    /// - `body`: the JSON body to send, when the endpoint takes one.
    ///
    /// # Returns
    ///
    /// `Ok(())` for a `2xx` response.
    ///
    /// # Errors
    ///
    /// Returns the mapped response error, and [`ProviderError::Unavailable`]
    /// when the host refuses or cannot complete the request.
    pub(crate) fn send_json_body<B: Serialize>(
        &self,
        method: &str,
        path: &str,
        body: &B,
    ) -> Result<(), ProviderError> {
        let body = serde_json::to_vec(body)
            .map_err(|error| ProviderError::Internal(format!("request body: {error}")))?;
        let response = self.send(method, path, Some(body))?;
        if hearthsound_plugin::http::is_success(&response) {
            Ok(())
        } else {
            Err(hearthsound_plugin::http::status_error(&response))
        }
    }

    /// Performs one request.
    ///
    /// # Parameters
    ///
    /// - `method`: HTTP method.
    /// - `path`: API path, without a leading slash.
    /// - `body`: the JSON body, when the endpoint takes one.
    ///
    /// # Returns
    ///
    /// The response, whatever its status.
    ///
    /// # Errors
    ///
    /// Maps the host's `http-error` to a provider error through the SDK.
    fn send(
        &self,
        method: &str,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<http::Response, ProviderError> {
        let mut headers = vec![("accept".to_string(), "application/json".to_string())];
        if let Some(token) = self.token {
            headers.push(("authorization".to_string(), format!("Token {token}")));
        }
        if body.is_some() {
            headers.push(("content-type".to_string(), "application/json".to_string()));
        }

        let request = http::Request {
            method: method.to_string(),
            url: crate::url::endpoint(self.base_url, path),
            headers,
            body,
            timeout_ms: Some(REQUEST_TIMEOUT_MS),
        };

        hearthsound_plugin::http::send(&request)
    }
}

/// Decodes a successful response body.
///
/// # Parameters
///
/// - `response`: the response to decode.
///
/// # Returns
///
/// The body deserialized into `T`.
///
/// # Errors
///
/// Returns the mapped response error for a non-`2xx` status and
/// [`ProviderError::Internal`] when the body is not JSON for `T`.
fn decode<T: DeserializeOwned>(response: http::Response) -> Result<T, ProviderError> {
    if !hearthsound_plugin::http::is_success(&response) {
        return Err(hearthsound_plugin::http::status_error(&response));
    }
    serde_json::from_slice(&response.body).map_err(|error| {
        ProviderError::Internal(format!("unexpected ListenBrainz response: {error}"))
    })
}

/// Decodes a response body that a `204 No Content` leaves empty.
///
/// # Parameters
///
/// - `response`: the response to decode.
///
/// # Returns
///
/// `None` for a `204` response, and the decoded body otherwise.
///
/// # Errors
///
/// Returns the mapped response error for a non-`2xx` status and
/// [`ProviderError::Internal`] when the body is not JSON for `T`.
pub(crate) fn decode_optional<T: DeserializeOwned>(
    response: http::Response,
) -> Result<Option<T>, ProviderError> {
    if response.status == 204 {
        return Ok(None);
    }
    decode(response).map(Some)
}

/// Sends one anonymous request to a fully built URL.
///
/// The labs API lives on its own host, so its endpoints are composed from
/// [`crate::url::LABS_BASE_URL`] rather than from the configured API base URL.
///
/// # Parameters
///
/// - `url`: the absolute URL to fetch.
///
/// # Returns
///
/// The decoded response body.
///
/// # Errors
///
/// Returns the mapped response error, and [`ProviderError::Unavailable`] when
/// the host refuses or cannot complete the request.
pub(crate) fn get_absolute<T: DeserializeOwned>(url: &str) -> Result<T, ProviderError> {
    let request = hearthsound_plugin::http::get(url);
    let response = hearthsound_plugin::http::send(&request)?;
    decode(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a response with the given status and body.
    fn response(status: u16, body: &str) -> http::Response {
        http::Response {
            status,
            headers: Vec::new(),
            body: body.as_bytes().to_vec(),
        }
    }

    #[test]
    fn decode_reads_a_successful_body() {
        let value: serde_json::Value = decode(response(200, "{\"valid\":true}")).unwrap();
        assert_eq!(value["valid"], serde_json::json!(true));
    }

    #[test]
    fn decode_maps_failures_to_provider_errors() {
        assert!(matches!(
            decode::<serde_json::Value>(response(401, "invalid token")),
            Err(ProviderError::Unauthorized(_))
        ));
        assert!(matches!(
            decode::<serde_json::Value>(response(404, "")),
            Err(ProviderError::NotFound)
        ));
        assert!(matches!(
            decode::<serde_json::Value>(response(503, "down")),
            Err(ProviderError::Unavailable(_))
        ));
    }

    #[test]
    fn decode_reports_a_body_of_the_wrong_shape() {
        assert!(matches!(
            decode::<Vec<String>>(response(200, "{\"not\":\"a list\"}")),
            Err(ProviderError::Internal(_))
        ));
    }

    #[test]
    fn decode_optional_accepts_a_204_without_a_body() {
        let value: Option<serde_json::Value> = decode_optional(response(204, "")).unwrap();
        assert!(value.is_none());
    }

    #[test]
    fn decode_optional_still_reads_a_body() {
        let value: Option<serde_json::Value> = decode_optional(response(200, "[1,2]")).unwrap();
        assert_eq!(value, Some(serde_json::json!([1, 2])));
    }
}
