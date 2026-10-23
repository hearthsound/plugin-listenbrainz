//! The user-scoped configuration the server stores for a linked account.
//!
//! The host hands a provider the decrypted inner config object of the user's
//! stored envelope as a JSON string. For ListenBrainz that object holds the
//! user token every authenticated call needs.

use hearthsound_plugin::types::{ProviderError, UserContext};
use serde::Deserialize;

/// A linked ListenBrainz account's stored settings.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct UserConfig {
    /// The user token ListenBrainz issued.
    pub(crate) token: String,
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
}
