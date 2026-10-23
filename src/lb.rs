//! Serde types for the ListenBrainz API responses and request bodies.
//!
//! The types mirror the JSON shapes ListenBrainz documents, including the parts
//! the plugin does not use, so an unexpected field never fails a call. They are
//! pure data: [`crate::scrobble`] and [`crate::discovery`] turn them into
//! provider records.

use std::collections::HashMap;

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

/// Response of `GET /1/cf/recommendation/user/{user}/recording`.
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct Recommendation {
    /// The recommendation page.
    #[serde(default)]
    pub(crate) payload: RecommendationPayload,
}

/// The recommendation page of [`Recommendation`].
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct RecommendationPayload {
    /// Recommended recordings, best first.
    #[serde(default)]
    pub(crate) mbids: Vec<ScoredMbid>,
}

/// One recommended recording with its collaborative-filtering score.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ScoredMbid {
    /// MusicBrainz recording id.
    pub(crate) recording_mbid: String,
    /// The recommendation score.
    #[serde(default)]
    pub(crate) score: Option<f64>,
}

/// Response of `GET` and `POST /1/metadata/recording/`, keyed by recording mbid.
pub(crate) type Metadata = HashMap<String, MetadataEntry>;

/// The metadata known for one recording.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MetadataEntry {
    /// The recording itself.
    #[serde(default)]
    pub(crate) recording: Option<RecordingMetadata>,
    /// The artist credit.
    #[serde(default)]
    pub(crate) artist: Option<ArtistMetadata>,
    /// The release the recording appears on.
    #[serde(default)]
    pub(crate) release: Option<ReleaseMetadata>,
}

/// The recording member of [`MetadataEntry`].
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct RecordingMetadata {
    /// Recording title.
    #[serde(default)]
    pub(crate) name: Option<String>,
    /// Recording length in milliseconds.
    #[serde(default)]
    pub(crate) length: Option<u32>,
}

/// The artist member of [`MetadataEntry`].
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ArtistMetadata {
    /// The full credit string.
    #[serde(default)]
    pub(crate) name: Option<String>,
    /// The credited artists in order.
    #[serde(default)]
    pub(crate) artists: Vec<CreditedArtist>,
}

/// One artist of an artist credit.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreditedArtist {
    /// The artist's name as credited.
    #[serde(default)]
    pub(crate) name: Option<String>,
    /// MusicBrainz artist id.
    #[serde(default)]
    pub(crate) artist_mbid: Option<String>,
}

/// The release member of [`MetadataEntry`].
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct ReleaseMetadata {
    /// Release title.
    #[serde(default)]
    pub(crate) name: Option<String>,
}

/// One row of the labs `similar-artists` dataset.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SimilarArtist {
    /// MusicBrainz artist id.
    #[serde(default)]
    pub(crate) artist_mbid: Option<String>,
    /// Artist name.
    #[serde(default)]
    pub(crate) name: Option<String>,
    /// Session-based similarity score.
    #[serde(default)]
    pub(crate) score: Option<f64>,
}

/// One row of the labs `similar-recordings` dataset.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SimilarRecording {
    /// MusicBrainz recording id.
    #[serde(default)]
    pub(crate) recording_mbid: Option<String>,
    /// Recording title.
    #[serde(default)]
    pub(crate) recording_name: Option<String>,
    /// The artist credit string.
    #[serde(default)]
    pub(crate) artist_credit_name: Option<String>,
    /// MusicBrainz artist ids of the credit.
    #[serde(default, alias = "[artist_credit_mbids]")]
    pub(crate) artist_credit_mbids: Option<Vec<String>>,
    /// Release title.
    #[serde(default)]
    pub(crate) release_name: Option<String>,
    /// Session-based similarity score.
    #[serde(default)]
    pub(crate) score: Option<f64>,
}

/// One row of `GET /1/popularity/top-recordings-for-artist/{mbid}`.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PopularRecording {
    /// MusicBrainz recording id.
    #[serde(default)]
    pub(crate) recording_mbid: Option<String>,
    /// Recording title.
    #[serde(default)]
    pub(crate) recording_name: Option<String>,
    /// The artist credit string.
    #[serde(default)]
    pub(crate) artist_name: Option<String>,
    /// Release title.
    #[serde(default)]
    pub(crate) release_name: Option<String>,
    /// Recording length in milliseconds.
    #[serde(default)]
    pub(crate) length: Option<u32>,
}

/// Response of `GET /1/user/{user}/playlists/createdfor`.
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct CreatedFor {
    /// The generated playlists.
    #[serde(default)]
    pub(crate) playlists: Vec<CreatedForEntry>,
}

/// One generated playlist entry.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CreatedForEntry {
    /// The playlist metadata.
    pub(crate) playlist: PlaylistMetadata,
}

/// The metadata member of a created-for playlist.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PlaylistMetadata {
    /// Playlist title.
    #[serde(default)]
    pub(crate) title: Option<String>,
    /// Playlist description.
    #[serde(default)]
    pub(crate) annotation: Option<String>,
    /// Canonical URL of the playlist.
    #[serde(default)]
    pub(crate) identifier: Option<String>,
    /// JSPF extensions.
    #[serde(default)]
    pub(crate) extension: Option<PlaylistExtension>,
}

/// The extensions member of a playlist.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PlaylistExtension {
    /// The MusicBrainz JSPF playlist extension.
    #[serde(rename = "https://musicbrainz.org/doc/jspf#playlist")]
    pub(crate) playlist: Option<PlaylistExtensionBody>,
}

/// The MusicBrainz JSPF playlist extension body.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PlaylistExtensionBody {
    /// Fields the generating bot adds.
    #[serde(default)]
    pub(crate) additional_metadata: Option<PlaylistAdditionalMetadata>,
}

/// The additional metadata of a generated playlist.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct PlaylistAdditionalMetadata {
    /// Which generator produced the playlist.
    #[serde(default)]
    pub(crate) algorithm_metadata: Option<AlgorithmMetadata>,
    /// When the playlist retires, as an ISO 8601 timestamp without a zone.
    #[serde(default)]
    pub(crate) expires_at: Option<String>,
}

/// The generator of a playlist.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct AlgorithmMetadata {
    /// Stable name of the generator, for example `weekly-jams`.
    #[serde(default)]
    pub(crate) source_patch: Option<String>,
}

/// A JSPF playlist document.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct JspfDocument {
    /// The playlist.
    pub(crate) playlist: JspfPlaylist,
}

/// The playlist member of a JSPF document.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct JspfPlaylist {
    /// The tracks, in playlist order.
    #[serde(default)]
    pub(crate) track: Vec<JspfTrack>,
}

/// One track of a JSPF playlist.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct JspfTrack {
    /// Track title.
    #[serde(default)]
    pub(crate) title: Option<String>,
    /// Artist credit string.
    #[serde(default)]
    pub(crate) creator: Option<String>,
    /// Album title.
    #[serde(default)]
    pub(crate) album: Option<String>,
    /// Track length in milliseconds.
    #[serde(default)]
    pub(crate) duration: Option<u32>,
    /// Recording URIs; one element for a single recording.
    #[serde(default, deserialize_with = "string_or_vec")]
    pub(crate) identifier: Vec<String>,
    /// JSPF extensions.
    #[serde(default)]
    pub(crate) extension: Option<TrackExtension>,
}

/// The extensions member of a JSPF track.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TrackExtension {
    /// The MusicBrainz JSPF track extension.
    #[serde(rename = "https://musicbrainz.org/doc/jspf#track")]
    pub(crate) track: Option<TrackExtensionBody>,
}

/// The MusicBrainz JSPF track extension body.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TrackExtensionBody {
    /// Fields ListenBrainz adds.
    #[serde(default)]
    pub(crate) additional_metadata: Option<TrackAdditionalMetadata>,
}

/// The additional metadata of a JSPF track.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TrackAdditionalMetadata {
    /// The artist credit, artist by artist.
    #[serde(default)]
    pub(crate) artists: Vec<TrackCreditArtist>,
}

/// One artist of a JSPF track credit.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct TrackCreditArtist {
    /// The artist's name as credited.
    #[serde(default)]
    pub(crate) artist_credit_name: Option<String>,
    /// MusicBrainz artist id.
    #[serde(default)]
    pub(crate) artist_mbid: Option<String>,
}

/// Accepts either one string or a list of strings.
///
/// JSPF allows an `identifier` to be a single URI, while ListenBrainz returns a
/// one-element list for some documents and a plain string for others.
///
/// # Parameters
///
/// - `deserializer`: the value to read.
///
/// # Returns
///
/// The value as a list, empty when the member is absent.
///
/// # Errors
///
/// Returns the deserializer's error when the value is neither form.
fn string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    /// Either form of a JSPF identifier.
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        /// A single URI.
        One(String),
        /// A list of URIs.
        Many(Vec<String>),
    }

    match Option::<OneOrMany>::deserialize(deserializer)? {
        None => Ok(Vec::new()),
        Some(OneOrMany::One(value)) => Ok(vec![value]),
        Some(OneOrMany::Many(values)) => Ok(values),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses one embedded fixture.
    fn fixture<T: serde::de::DeserializeOwned>(name: &str) -> T {
        let text = match name {
            "validate_token" => include_str!("fixtures/validate_token.json"),
            "validate_token_invalid" => include_str!("fixtures/validate_token_invalid.json"),
            "recommendation" => include_str!("fixtures/recommendation.json"),
            "metadata_recording" => include_str!("fixtures/metadata_recording.json"),
            "similar_artists" => include_str!("fixtures/similar_artists.json"),
            "similar_recordings" => include_str!("fixtures/similar_recordings.json"),
            "top_recordings" => include_str!("fixtures/top_recordings.json"),
            "createdfor" => include_str!("fixtures/createdfor.json"),
            "playlist" => include_str!("fixtures/playlist.json"),
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

    #[test]
    fn recommendation_reads_the_scored_mbids() {
        let answer: Recommendation = fixture("recommendation");
        assert_eq!(answer.payload.mbids.len(), 2);
        assert_eq!(
            answer.payload.mbids[0].recording_mbid,
            "3c525387-0f7e-47e4-a731-64ce87819717"
        );
        assert_eq!(answer.payload.mbids[0].score, Some(9.345));
    }

    #[test]
    fn metadata_reads_the_name_artist_and_release() {
        let answer: Metadata = fixture("metadata_recording");
        let entry = &answer["3c525387-0f7e-47e4-a731-64ce87819717"];
        let recording = entry.recording.as_ref().unwrap();
        assert_eq!(recording.name.as_deref(), Some("Nothing"));
        assert_eq!(recording.length, Some(178_800));
        let artist = entry.artist.as_ref().unwrap();
        assert_eq!(artist.name.as_deref(), Some("Christian Loffler feat. Adna"));
        assert_eq!(artist.artists.len(), 2);
        assert_eq!(
            entry.release.as_ref().unwrap().name.as_deref(),
            Some("Felt")
        );
    }

    #[test]
    fn similar_artists_reads_every_row() {
        let rows: Vec<SimilarArtist> = fixture("similar_artists");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name.as_deref(), Some("Nirvana"));
        assert_eq!(rows[0].score, Some(11_156.0));
    }

    #[test]
    fn similar_recordings_accepts_a_bracketed_artist_list() {
        let rows: Vec<SimilarRecording> = fixture("similar_recordings");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].recording_name.as_deref(), Some("Familiar"));
        assert_eq!(rows[0].artist_credit_name.as_deref(), Some("Nils Frahm"));
        assert_eq!(
            rows[0].artist_credit_mbids.as_deref(),
            Some(&["d03a54bc-6e07-4c04-a8d4-7fbf2bd07a01".to_string()][..])
        );
    }

    #[test]
    fn popular_recordings_read_the_play_counts() {
        let rows: Vec<PopularRecording> = fixture("top_recordings");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].recording_name.as_deref(), Some("Closer"));
        assert_eq!(rows[0].length, Some(373_133));
    }

    #[test]
    fn createdfor_reads_the_source_patch_and_expiry() {
        let answer: CreatedFor = fixture("createdfor");
        let playlist = &answer.playlists[0].playlist;
        assert_eq!(playlist.title.as_deref(), Some("Weekly Jams for rob"));
        assert_eq!(playlist.annotation.as_deref(), Some("A weekly playlist."));
        let body = playlist
            .extension
            .as_ref()
            .unwrap()
            .playlist
            .as_ref()
            .unwrap()
            .additional_metadata
            .as_ref()
            .unwrap();
        assert_eq!(
            body.algorithm_metadata
                .as_ref()
                .unwrap()
                .source_patch
                .as_deref(),
            Some("weekly-jams")
        );
        assert_eq!(
            body.expires_at.as_deref(),
            Some("2026-11-05T22:05:18.572700")
        );
    }

    #[test]
    fn jspf_reads_tracks_and_their_identifiers() {
        let document: JspfDocument = fixture("playlist");
        assert_eq!(document.playlist.track.len(), 2);
        let first = &document.playlist.track[0];
        assert_eq!(first.title.as_deref(), Some("Nothing"));
        assert_eq!(
            first.identifier,
            vec!["https://musicbrainz.org/recording/3c525387-0f7e-47e4-a731-64ce87819717"]
        );
        assert_eq!(
            first
                .extension
                .as_ref()
                .unwrap()
                .track
                .as_ref()
                .unwrap()
                .additional_metadata
                .as_ref()
                .unwrap()
                .artists
                .len(),
            2
        );
    }

    #[test]
    fn identifiers_accept_one_string_and_an_absent_member() {
        let document: JspfDocument = serde_json::from_str(
            r#"{"playlist":{"track":[
                {"title":"One","identifier":"https://musicbrainz.org/recording/abc"},
                {"title":"Two"}
            ]}}"#,
        )
        .unwrap();
        assert_eq!(document.playlist.track[0].identifier.len(), 1);
        assert!(document.playlist.track[1].identifier.is_empty());
    }
}
