//! Serde types for the ListenBrainz API request and response bodies.
//!
//! The types mirror the JSON shapes ListenBrainz documents, including the parts
//! the plugin does not use, so an unexpected field never fails a call. They are
//! pure data: [`crate::scrobble`] turns them into provider records.

use serde::{Deserialize, Serialize};

/// Response of `GET /1/validate-token`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct ValidateToken {
    /// Whether the token belongs to a user.
    pub(crate) valid: bool,
    /// The user's MusicBrainz name, present only for a valid token.
    #[serde(default)]
    pub(crate) user_name: Option<String>,
}

/// One listen in the submission form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct RawListen {
    /// When playback started, unix seconds; omitted for a now-playing listen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) listened_at: Option<i64>,
    /// The track the listen is about.
    pub(crate) track_metadata: RawTrackMetadata,
}

/// The track metadata member of a listen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct RawTrackMetadata {
    /// Artist credit string.
    pub(crate) artist_name: String,
    /// Track title.
    pub(crate) track_name: String,
    /// Album title, when the track has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) release_name: Option<String>,
    /// Identifiers and player information.
    pub(crate) additional_info: AdditionalInfo,
}

/// The additional information member of a listen.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct AdditionalInfo {
    /// MusicBrainz recording id, when the server knows one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) recording_mbid: Option<String>,
    /// MusicBrainz release id, when the server knows one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) release_mbid: Option<String>,
    /// MusicBrainz artist ids in credit order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) artist_mbids: Vec<String>,
    /// Track duration in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) duration_ms: Option<u32>,
    /// One-based track number on its album.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) tracknumber: Option<u16>,
    /// The application that submitted the listen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) submission_client: Option<String>,
    /// The player that played the track.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) media_player: Option<String>,
}

/// Body of `POST /1/submit-listens`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SubmitPaylod {
    /// `single`, `import` or `playing_now`.
    pub(crate) listen_type: &'static str,
    /// The listens the submission carries.
    pub(crate) payload: Vec<RawListen>,
}

/// Body of `POST /1/feedback/recording-feedback`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct Feedback {
    /// MusicBrainz recording id the feedback is about.
    pub(crate) recording_mbid: String,
    /// `1` for love, `-1` for hate and `0` to clear.
    pub(crate) score: i8,
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
    fn validate_token_reads_a_valid_answer() {
        let answer: ValidateToken = fixture("validate_token");
        assert!(answer.valid);
        assert_eq!(answer.user_name.as_deref(), Some("fixture-user"));
    }

    #[test]
    fn validate_token_reads_an_invalid_answer() {
        let answer: ValidateToken = fixture("validate_token_invalid");
        assert!(!answer.valid);
        assert!(answer.user_name.is_none());
    }
}
