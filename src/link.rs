//! Account linking: the user token is checked with ListenBrainz before it is
//! stored, and the user name it belongs to is recorded with it.
//!
//! ListenBrainz has no browser authorization flow for third-party
//! applications, so the plugin links with the `config` method only: the user
//! pastes the token from their ListenBrainz settings page.

use hearthsound_plugin::scrobble_discovery_link_provider::exports::hearthsound::plugin::account_link::Account;
use hearthsound_plugin::types::ProviderError;

use crate::api::Client;
use crate::lb;
use crate::user::UserConfig;

/// Reads the answer ListenBrainz gives for a user token.
///
/// # Parameters
///
/// - `client`: the API client, carrying the token to check.
///
/// # Returns
///
/// The answer: whether the token is valid and, when it is, the MusicBrainz name
/// of the user it belongs to.
///
/// # Errors
///
/// Returns the provider error of the request.
pub(crate) fn validate_token(client: &Client<'_>) -> Result<lb::ValidateToken, ProviderError> {
    client.get("1/validate-token")
}

/// Turns a token-validation answer into the linked account.
///
/// # Parameters
///
/// - `token`: the token that was checked.
/// - `answer`: what ListenBrainz answered.
///
/// # Returns
///
/// The account: the MusicBrainz name to show, and the user config to store,
/// which holds the token and the name the user endpoints address.
///
/// # Errors
///
/// Returns [`ProviderError::Unauthorized`] when ListenBrainz does not accept the
/// token, and [`ProviderError::Unavailable`] when it accepts the token without
/// naming the user.
pub(crate) fn account_from_answer(
    token: &str,
    answer: lb::ValidateToken,
) -> Result<Account, ProviderError> {
    if !answer.valid {
        return Err(ProviderError::Unauthorized(
            "ListenBrainz rejected the user token".to_string(),
        ));
    }

    let user_name = answer
        .user_name
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            ProviderError::Unavailable(
                "ListenBrainz validated the token without naming the user".to_string(),
            )
        })?;

    let stored = UserConfig {
        token: token.to_string(),
        user_name: Some(user_name.clone()),
    };

    Ok(Account {
        name: user_name,
        config: stored.encode()?,
    })
}

/// Verifies a user config and describes the linked account.
///
/// # Parameters
///
/// - `base_url`: base URL of the ListenBrainz API.
/// - `config`: the user config the listener entered, as JSON.
///
/// # Returns
///
/// The stored form of the config, with the user name added, and the account
/// name to show.
///
/// # Errors
///
/// Returns [`ProviderError::InvalidConfig`] when the config has no usable token,
/// [`ProviderError::Unauthorized`] when ListenBrainz rejects it, and the
/// provider error of the request.
pub(crate) fn verify_account(base_url: &str, config: &str) -> Result<Account, ProviderError> {
    let entered: UserConfig = serde_json::from_str(config)
        .map_err(|error| ProviderError::InvalidConfig(format!("user config: {error}")))?;
    let token = entered.token.trim();
    if token.is_empty() {
        return Err(ProviderError::InvalidConfig(
            "user config: token is empty".to_string(),
        ));
    }

    let client = Client {
        base_url,
        token: Some(token),
    };
    let answer = validate_token(&client)?;

    account_from_answer(token, answer)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses one embedded fixture.
    fn fixture<T: serde::de::DeserializeOwned>(name: &str) -> T {
        let text = match name {
            "validate_token" => include_str!("fixtures/validate_token.json"),
            "validate_token_invalid" => include_str!("fixtures/validate_token_invalid.json"),
            other => panic!("unknown fixture {other}"),
        };
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn a_valid_token_names_the_account_and_stores_its_user_name() {
        let answer: lb::ValidateToken = fixture("validate_token");
        let account = account_from_answer("good", answer).unwrap();

        assert_eq!(account.name, "fixture-user");
        assert_eq!(
            account.config,
            "{\"token\":\"good\",\"userName\":\"fixture-user\"}"
        );
    }

    #[test]
    fn an_invalid_token_is_unauthorized() {
        let answer: lb::ValidateToken = fixture("validate_token_invalid");
        assert!(matches!(
            account_from_answer("bad", answer),
            Err(ProviderError::Unauthorized(_))
        ));
    }

    #[test]
    fn a_validation_without_a_user_name_is_unavailable() {
        let answer = lb::ValidateToken {
            valid: true,
            user_name: None,
        };
        assert!(matches!(
            account_from_answer("good", answer),
            Err(ProviderError::Unavailable(_))
        ));
    }

    /// A config with an empty token is rejected before any request is made.
    #[test]
    fn an_empty_token_is_refused() {
        assert!(matches!(
            verify_account("https://api.listenbrainz.org", "{\"token\":\"\"}"),
            Err(ProviderError::InvalidConfig(_))
        ));
        assert!(matches!(
            verify_account("https://api.listenbrainz.org", "{\"token\":\"  \"}"),
            Err(ProviderError::InvalidConfig(_))
        ));
    }

    /// A config that is not JSON for the settings is refused.
    #[test]
    fn broken_json_is_refused() {
        assert!(matches!(
            verify_account("https://api.listenbrainz.org", "not json"),
            Err(ProviderError::InvalidConfig(_))
        ));
    }
}
