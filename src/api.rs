//! HTTP access to the ListenBrainz API through the `host-http` host import.
//!
//! This is the only module that touches the network. It builds one request,
//! sends it, and either reports success or maps the response to a provider
//! error; endpoint composition and payloads live in [`crate::scrobble`].
//!
//! ListenBrainz reports an invalid token with HTTP 401 and a rate limit with
//! HTTP 429, so the SDK's [`status_error`](hearthsound_plugin::http::status_error)
//! mapping applies unchanged.

use hearthsound_plugin::host::http;
use hearthsound_plugin::types::ProviderError;
use serde::Serialize;

/// Wall-clock budget for one ListenBrainz request, in milliseconds.
///
/// The host clamps this to its own ceiling and to the remaining call deadline.
const REQUEST_TIMEOUT_MS: u32 = 10_000;

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

    /// Sends a request whose success carries no body the plugin reads.
    ///
    /// # Parameters
    ///
    /// - `method`: HTTP method.
    /// - `path`: API path, without a leading slash.
    /// - `body`: the JSON body to send.
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
