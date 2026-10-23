//! Serde types for the ListenBrainz API request and response bodies.
//!
//! The types mirror the JSON shapes ListenBrainz documents, including the parts
//! the plugin does not use, so an unexpected field never fails a call. They are
//! pure data: [`crate::scrobble`] turns them into provider records.

use serde::{Deserialize, Serialize};

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
