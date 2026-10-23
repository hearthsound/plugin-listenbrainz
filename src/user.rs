//! The user-scoped configuration the server stores for a linked account.
//!
//! The host hands a provider the decrypted inner config object of the user's
//! stored envelope as a JSON string. For ListenBrainz that object holds the
//! user token every authenticated call needs and the user name the
//! recommendation and playlist endpoints address.

use hearthsound_plugin::types::{ProviderError, UserContext};
use serde::{Deserialize, Serialize};

/// A linked ListenBrainz account's stored settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UserConfig {
    /// The user token ListenBrainz issued.
    pub(crate) token: String,
    /// The user's MusicBrainz name, recorded when the token was verified.
    #[serde(default, rename = "userName")]
    pub(crate) user_name: Option<String>,
}

impl UserConfig {
    /// Parses the user config a provider call carries.
    ///
    /// # Parameters
    ///
    /// - `user`: the user context the host passed.
    ///
    /// # Returns
    ///
    /// The parsed settings.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Unauthorized`] when the user has not linked an
    /// account (the context carries no config) and
    /// [`ProviderError::InvalidConfig`] when the config is not JSON for these
    /// settings or holds an empty token.
    pub(crate) fn from_context(user: &UserContext) -> Result<Self, ProviderError> {
        let Some(raw) = user.config.as_deref() else {
            return Err(ProviderError::Unauthorized(
                "the user has not linked a ListenBrainz account".to_string(),
            ));
        };

        let config: Self = serde_json::from_str(raw)
            .map_err(|error| ProviderError::InvalidConfig(format!("user config: {error}")))?;
        if config.token.trim().is_empty() {
            return Err(ProviderError::InvalidConfig(
                "user config: token is empty".to_string(),
            ));
        }

        Ok(config)
    }

    /// Serializes the settings for the host to store.
    ///
    /// # Returns
    ///
    /// The JSON object the account-link interface returns.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::Internal`] when the settings cannot be
    /// serialized, which cannot happen for this record.
    pub(crate) fn encode(&self) -> Result<String, ProviderError> {
        serde_json::to_string(self)
            .map_err(|error| ProviderError::Internal(format!("user config: {error}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a user context with the given config.
    fn context(config: Option<&str>) -> UserContext {
        UserContext {
            user_id: "0b5c1d0e-7f4b-4e2e-9f6a-1c2d3e4f5a6b".to_string(),
            config: config.map(str::to_string),
        }
    }

    #[test]
    fn parses_a_stored_account() {
        let config = UserConfig::from_context(&context(Some(
            "{\"token\":\"abc\",\"userName\":\"fixture-user\"}",
        )))
        .unwrap();

        assert_eq!(config.token, "abc");
        assert_eq!(config.user_name.as_deref(), Some("fixture-user"));
    }

    #[test]
    fn a_missing_config_means_the_account_is_not_linked() {
        assert!(matches!(
            UserConfig::from_context(&context(None)),
            Err(ProviderError::Unauthorized(_))
        ));
    }

    #[test]
    fn rejects_an_empty_token_and_broken_json() {
        assert!(matches!(
            UserConfig::from_context(&context(Some("{\"token\":\"\"}"))),
            Err(ProviderError::InvalidConfig(_))
        ));
        assert!(matches!(
            UserConfig::from_context(&context(Some("[]"))),
            Err(ProviderError::InvalidConfig(_))
        ));
    }

    #[test]
    fn encoding_round_trips_and_uses_the_camel_case_name() {
        let config = UserConfig {
            token: "abc".to_string(),
            user_name: Some("fixture-user".to_string()),
        };
        let encoded = config.encode().unwrap();
        assert_eq!(encoded, "{\"token\":\"abc\",\"userName\":\"fixture-user\"}");
        assert_eq!(
            UserConfig::from_context(&context(Some(&encoded))).unwrap(),
            config
        );
    }
}
