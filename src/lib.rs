//! ListenBrainz plugin: a Hearthsound scrobbling and discovery provider over
//! the ListenBrainz API.
//!
//! The plugin implements `scrobble` (now playing, finished listens and
//! love/hate feedback), `discovery` (similar artists and tracks, an artist's
//! top recordings, collaborative-filtering recommendations and the playlists
//! ListenBrainz generates for a user) and the `account-link` capability that
//! connects a listener's account with their user token.
//!
//! The server settings are `baseUrl`, which defaults to the public API and can
//! be pointed at a mirror or a test double, and `labsBaseUrl`, which points the
//! similarity datasets at a labs mirror or a test double (those requests carry
//! no credential). The user setting is the ListenBrainz token, which the plugin
//! checks before it is stored.
//!
//! The pure logic sits in public modules ([`url`], [`lb`], [`time`]); the
//! binding-coupled glue that drives it stays private.
//!
//! # Tests
//!
//! The pure logic is unit-tested on the host target against captured response
//! fixtures, so no test touches the network:
//!
//! ```text
//! cargo test
//! ```
//!
//! Rustdoc examples in those modules are illustrative `text` blocks rather than
//! doctests: the crate is a `cdylib` for `wasm32-wasip2`, so there is no host
//! library target for a doctest to link against. Every example appears as an
//! assertion in the unit tests of the module that carries it.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]

use hearthsound_plugin::scrobble_discovery_link_provider::exports::hearthsound::plugin::account_link::{
    Account, Guest as AccountLinkGuest, LinkMethod,
};
use hearthsound_plugin::scrobble_discovery_link_provider::exports::hearthsound::plugin::discovery::{
    ArtistSeed, GeneratedPlaylist, Guest as DiscoveryGuest, SuggestedArtist, SuggestedTrack, TrackSeed,
};
use hearthsound_plugin::scrobble_discovery_link_provider::exports::hearthsound::plugin::lifecycle::Guest as LifecycleGuest;
use hearthsound_plugin::scrobble_discovery_link_provider::exports::hearthsound::plugin::scrobble::{
    FeedbackValue, Guest as ScrobbleGuest, Listen, ScrobbleLimits,
};
use hearthsound_plugin::types::{ProviderError, UserContext};

mod api;
mod discovery;
pub mod lb;
mod link;
mod scrobble;
pub mod time;
pub mod url;
mod user;

/// The plugin implementation.
struct ListenBrainzPlugin;

/// Server-scoped configuration of the plugin.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Config {
    /// Base URL of the ListenBrainz API.
    #[serde(default = "default_base_url")]
    base_url: String,
    /// Base URL of the ListenBrainz labs API, which serves the similarity
    /// datasets.
    #[serde(default = "default_labs_base_url")]
    labs_base_url: String,
}

/// Returns the base URL used when the instance configuration omits one.
fn default_base_url() -> String {
    url::DEFAULT_BASE_URL.to_string()
}

/// Returns the labs base URL used when the instance configuration omits one.
fn default_labs_base_url() -> String {
    url::DEFAULT_LABS_BASE_URL.to_string()
}

/// Normalizes and validates one configured base URL.
///
/// # Parameters
///
/// - `field`: the configuration member the value came from, named in the error
///   so the administrator knows which setting to correct.
/// - `value`: the configured value.
///
/// # Returns
///
/// The value trimmed and without a trailing slash.
///
/// # Errors
///
/// Returns the message naming `field` when the value is not an absolute `http`
/// or `https` URL.
fn checked_base_url(field: &str, value: &str) -> Result<String, String> {
    let normalized = url::normalize_base_url(value);

    if url::is_http_url(&normalized) {
        Ok(normalized)
    } else {
        Err(format!(
            "{field} must be an http or https URL, got `{value}`"
        ))
    }
}

/// Reads this instance's server configuration.
///
/// # Returns
///
/// The parsed configuration.
///
/// # Errors
///
/// Returns [`ProviderError::InvalidConfig`] when the configuration cannot be
/// read or deserialized.
fn config() -> Result<Config, ProviderError> {
    hearthsound_plugin::config::server_config()
}

/// Reads this instance's configuration and returns its API base URL.
///
/// # Returns
///
/// The configured base URL, trimmed and without a trailing slash.
///
/// # Errors
///
/// Returns [`ProviderError::InvalidConfig`] when the configuration cannot be
/// read or deserialized, and when the base URL is not an absolute `http` or
/// `https` URL.
fn base_url() -> Result<String, ProviderError> {
    let config = config()?;
    checked_base_url("baseUrl", &config.base_url).map_err(ProviderError::InvalidConfig)
}

/// Reads this instance's configuration and returns its labs base URL.
///
/// The similarity datasets answer on a different host than the API, so the two
/// `similar-*` calls compose their URLs from this value.
///
/// # Returns
///
/// The configured labs base URL, trimmed and without a trailing slash.
///
/// # Errors
///
/// Returns [`ProviderError::InvalidConfig`] when the configuration cannot be
/// read or deserialized, and when the labs base URL is not an absolute `http`
/// or `https` URL.
fn labs_base_url() -> Result<String, ProviderError> {
    let config = config()?;
    checked_base_url("labsBaseUrl", &config.labs_base_url).map_err(ProviderError::InvalidConfig)
}

/// Validates the plugin's server-scoped configuration.
///
/// # Parameters
///
/// - `server_config`: the configuration JSON as the administrator stored it.
///
/// # Returns
///
/// `Ok(())` when the configuration is usable.
///
/// # Errors
///
/// Returns one message per problem: JSON that does not deserialize into the
/// configuration, and a `baseUrl` or `labsBaseUrl` that is not an absolute
/// `http` or `https` URL.
fn validate_config(server_config: &str) -> Result<(), Vec<String>> {
    let config: Config = match serde_json::from_str(server_config) {
        Ok(config) => config,
        Err(error) => return Err(vec![format!("server config: {error}")]),
    };

    let errors = [
        checked_base_url("baseUrl", &config.base_url),
        checked_base_url("labsBaseUrl", &config.labs_base_url),
    ]
    .into_iter()
    .filter_map(Result::err)
    .collect::<Vec<_>>();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Implements the lifecycle interface.
impl LifecycleGuest for ListenBrainzPlugin {
    fn validate_config(server_config: String) -> Result<(), Vec<String>> {
        validate_config(&server_config)
    }

    fn health() -> Result<String, ProviderError> {
        let base = base_url()?;
        let client = api::Client {
            base_url: &base,
            token: None,
        };
        client.check("1/status/service-status")?;
        Ok("ListenBrainz API is reachable".to_string())
    }
}

/// Implements the scrobble interface.
impl ScrobbleGuest for ListenBrainzPlugin {
    fn get_limits() -> ScrobbleLimits {
        ScrobbleLimits {
            max_batch: scrobble::MAX_BATCH,
            min_interval_ms: scrobble::MIN_INTERVAL_MS,
            feedback: true,
        }
    }

    fn now_playing(user: UserContext, track: Listen) -> Result<(), ProviderError> {
        let config = user::UserConfig::from_context(&user)?;
        let base = base_url()?;
        let client = api::Client {
            base_url: &base,
            token: Some(&config.token),
        };
        scrobble::submit(&client, std::slice::from_ref(&track), true)
    }

    fn submit_listens(user: UserContext, listens: Vec<Listen>) -> Result<(), ProviderError> {
        let config = user::UserConfig::from_context(&user)?;
        let base = base_url()?;
        let client = api::Client {
            base_url: &base,
            token: Some(&config.token),
        };
        scrobble::submit(&client, &listens, false)
    }

    fn send_feedback(
        user: UserContext,
        track: Listen,
        value: FeedbackValue,
    ) -> Result<(), ProviderError> {
        let config = user::UserConfig::from_context(&user)?;
        let base = base_url()?;
        let client = api::Client {
            base_url: &base,
            token: Some(&config.token),
        };
        scrobble::feedback(&client, &track, value)
    }
}

/// Implements the discovery interface.
impl DiscoveryGuest for ListenBrainzPlugin {
    fn similar_tracks(
        user: Option<UserContext>,
        seed: TrackSeed,
        limit: u32,
    ) -> Result<Vec<SuggestedTrack>, ProviderError> {
        let _ = user;
        discovery::similar_tracks(&labs_base_url()?, &seed, limit)
    }

    fn similar_artists(
        user: Option<UserContext>,
        seed: ArtistSeed,
        limit: u32,
    ) -> Result<Vec<SuggestedArtist>, ProviderError> {
        let _ = user;
        discovery::similar_artists(&labs_base_url()?, &seed, limit)
    }

    fn artist_top_tracks(
        user: Option<UserContext>,
        artist: ArtistSeed,
        limit: u32,
    ) -> Result<Vec<SuggestedTrack>, ProviderError> {
        // The popularity endpoint needs a token. A listener who has not linked
        // an account has none, and the endpoint answers that caller with `401`,
        // so the question is answered with nothing rather than sent.
        let token = user
            .as_ref()
            .and_then(|user| user::UserConfig::from_context(user).ok())
            .map(|config| config.token);
        let base = base_url()?;
        let client = api::Client {
            base_url: &base,
            token: token.as_deref(),
        };
        discovery::artist_top_tracks(&client, &artist, limit)
    }

    fn recommended_tracks(
        user: UserContext,
        limit: u32,
    ) -> Result<Vec<SuggestedTrack>, ProviderError> {
        let config = user::UserConfig::from_context(&user)?;
        let user_name = config.require_name()?;
        let base = base_url()?;
        let client = api::Client {
            base_url: &base,
            token: Some(&config.token),
        };
        discovery::recommended_tracks(&client, user_name, limit)
    }

    fn playlists(user: UserContext) -> Result<Vec<GeneratedPlaylist>, ProviderError> {
        let config = user::UserConfig::from_context(&user)?;
        let user_name = config.require_name()?;
        let base = base_url()?;
        let client = api::Client {
            base_url: &base,
            token: Some(&config.token),
        };
        discovery::playlists(&client, user_name)
    }
}

/// Implements the account-link capability.
impl AccountLinkGuest for ListenBrainzPlugin {
    fn get_link_method() -> LinkMethod {
        LinkMethod::Config
    }

    fn verify_account(user: UserContext) -> Result<Account, ProviderError> {
        let config = user
            .config
            .as_deref()
            .ok_or_else(|| ProviderError::InvalidConfig("user config: missing".to_string()))?;
        link::verify_account(&base_url()?, config)
    }

    fn begin_link(user: UserContext, callback_url: String) -> Result<String, ProviderError> {
        let _ = (user, callback_url);
        Err(browser_link_unsupported())
    }

    fn complete_link(
        user: UserContext,
        query: Vec<(String, String)>,
    ) -> Result<Account, ProviderError> {
        let _ = (user, query);
        Err(browser_link_unsupported())
    }
}

/// Returns the error a browser link reports for this service.
///
/// # Returns
///
/// [`ProviderError::InvalidConfig`] naming the method the plugin does support.
fn browser_link_unsupported() -> ProviderError {
    ProviderError::InvalidConfig(
        "ListenBrainz links with a user token, not through the browser".to_string(),
    )
}

hearthsound_plugin::export_scrobble_discovery_link_provider!(ListenBrainzPlugin);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_base_url_is_the_public_api() {
        assert_eq!(default_base_url(), "https://api.listenbrainz.org");
    }

    #[test]
    fn the_default_labs_base_url_is_the_public_labs_host() {
        assert_eq!(default_labs_base_url(), "https://labs.api.listenbrainz.org");
    }

    #[test]
    fn a_configured_labs_base_url_is_read_from_its_camel_case_member() {
        let config: Config =
            serde_json::from_str("{\"labsBaseUrl\":\"https://labs.example.test/\"}").unwrap();

        assert_eq!(
            checked_base_url("labsBaseUrl", &config.labs_base_url).unwrap(),
            "https://labs.example.test"
        );
        assert_eq!(config.base_url, "https://api.listenbrainz.org");
    }

    #[test]
    fn the_default_configuration_is_valid() {
        assert!(validate_config("{}").is_ok());
    }

    #[test]
    fn a_non_http_base_url_is_refused() {
        let errors = validate_config("{\"baseUrl\":\"api.listenbrainz.org\"}").unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("baseUrl"));
    }

    #[test]
    fn a_non_http_labs_base_url_is_refused() {
        let errors =
            validate_config("{\"labsBaseUrl\":\"labs.api.listenbrainz.org\"}").unwrap_err();
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("labsBaseUrl"));
    }

    #[test]
    fn broken_configuration_json_is_refused() {
        assert!(validate_config("[").is_err());
    }

    #[test]
    fn the_link_method_is_the_user_token() {
        assert!(matches!(
            <ListenBrainzPlugin as AccountLinkGuest>::get_link_method(),
            LinkMethod::Config
        ));
    }
}
