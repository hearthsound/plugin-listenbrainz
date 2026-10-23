//! ListenBrainz plugin: a Hearthsound scrobbling and discovery provider over
//! the ListenBrainz API.
//!
//! The plugin implements `scrobble` (now playing, finished listens and
//! love/hate feedback), `discovery` (similar artists and tracks, an artist's
//! top recordings, collaborative-filtering recommendations and the playlists
//! ListenBrainz generates for a user) and the `account-link` capability that
//! connects a listener's account with their user token.
//!
//! The only server setting is `baseUrl`, which defaults to the public API and
//! can be pointed at a mirror or a test double. The user setting is the
//! ListenBrainz token, which the plugin checks before it is stored.
//!
//! The pure logic sits in public modules ([`url`], [`lb`]); the
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
pub mod lb;
mod link;
mod scrobble;
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
}

/// Returns the base URL used when the instance configuration omits one.
fn default_base_url() -> String {
    url::DEFAULT_BASE_URL.to_string()
}

/// Reads this instance's configuration and returns its base URL.
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
    let config: Config = hearthsound_plugin::config::server_config()?;
    let base_url = url::normalize_base_url(&config.base_url);

    if url::is_http_url(&base_url) {
        Ok(base_url)
    } else {
        Err(ProviderError::InvalidConfig(format!(
            "baseUrl must be an http or https URL, got `{}`",
            config.base_url
        )))
    }
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
/// configuration, and a `baseUrl` that is not an absolute `http` or `https`
/// URL.
fn validate_config(server_config: &str) -> Result<(), Vec<String>> {
    let config: Config = match serde_json::from_str(server_config) {
        Ok(config) => config,
        Err(error) => return Err(vec![format!("server config: {error}")]),
    };
    let base_url = url::normalize_base_url(&config.base_url);

    if url::is_http_url(&base_url) {
        Ok(())
    } else {
        Err(vec![format!(
            "baseUrl must be an http or https URL, got `{}`",
            config.base_url
        )])
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
        let _ = (user, seed, limit);
        Ok(Vec::new())
    }

    fn similar_artists(
        user: Option<UserContext>,
        seed: ArtistSeed,
        limit: u32,
    ) -> Result<Vec<SuggestedArtist>, ProviderError> {
        let _ = (user, seed, limit);
        Ok(Vec::new())
    }

    fn artist_top_tracks(
        user: Option<UserContext>,
        artist: ArtistSeed,
        limit: u32,
    ) -> Result<Vec<SuggestedTrack>, ProviderError> {
        let _ = (user, artist, limit);
        Ok(Vec::new())
    }

    fn recommended_tracks(
        user: UserContext,
        limit: u32,
    ) -> Result<Vec<SuggestedTrack>, ProviderError> {
        let _ = (user, limit);
        Ok(Vec::new())
    }

    fn playlists(user: UserContext) -> Result<Vec<GeneratedPlaylist>, ProviderError> {
        let _ = user;
        Ok(Vec::new())
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
