//! Scrobbling: finished listens, now playing and love/hate feedback.
//!
//! The module turns the provider records the host passes in into ListenBrainz
//! submission documents and posts them. The translation is a pure function so
//! its shape is unit-tested without a network; only [`submit`] and [`feedback`]
//! send anything.

use hearthsound_plugin::scrobble_discovery_link_provider::exports::hearthsound::plugin::scrobble::{
    FeedbackValue, Listen,
};
use hearthsound_plugin::types::{IdPair, ProviderError};

use crate::api::Client;
use crate::lb;

/// Most listens one submission may carry.
///
/// ListenBrainz publishes a per-request listen cap; the value is the ceiling the
/// plugin asks the host to batch up to.
pub(crate) const MAX_BATCH: u32 = 100;

/// Least time between two calls to ListenBrainz, in milliseconds.
pub(crate) const MIN_INTERVAL_MS: u32 = 1_000;

/// Name the plugin reports as the submitting application.
const SUBMISSION_CLIENT: &str = "Hearthsound";

/// Namespace of a MusicBrainz recording identifier.
const MB_RECORDING: &str = "mb-recording";

/// Namespace of a MusicBrainz release identifier.
const MB_RELEASE: &str = "mb-release";

/// Namespace of a MusicBrainz artist identifier.
const MB_ARTIST: &str = "mb-artist";

/// Returns the value of the first id in `ids` with the given namespace.
///
/// # Parameters
///
/// - `ids`: the identifiers to search.
/// - `namespace`: the namespace to match.
///
/// # Returns
///
/// The first matching value, or `None` when no id uses the namespace.
pub(crate) fn id_value<'a>(ids: &'a [IdPair], namespace: &str) -> Option<&'a str> {
    ids.iter()
        .find(|id| id.namespace == namespace)
        .map(|id| id.value.as_str())
}

/// Builds the submission document for a set of listens.
///
/// # Parameters
///
/// - `listens`: the listens to submit.
/// - `playing_now`: whether the document announces the start of playback
///   rather than finished listens.
///
/// # Returns
///
/// A `playing_now` document for a single listen when `playing_now` is set, a
/// `single` document for exactly one finished listen, and an `import` document
/// for more than one. ListenBrainz accepts an `import` document with one listen
/// too, but `single` states the intent and keeps a retry of one listen cheap.
pub(crate) fn submission(listens: &[Listen], playing_now: bool) -> lb::SubmitPaylod {
    let listen_type = if playing_now {
        "playing_now"
    } else if listens.len() == 1 {
        "single"
    } else {
        "import"
    };

    lb::SubmitPaylod {
        listen_type,
        payload: listens
            .iter()
            .map(|listen| raw_listen(listen, playing_now))
            .collect(),
    }
}

/// Translates one listen into its submission form.
///
/// # Parameters
///
/// - `listen`: the listen the host recorded.
/// - `playing_now`: whether the timestamp must be omitted.
///
/// # Returns
///
/// The listen document ListenBrainz accepts. The artist credit is joined with
/// commas because ListenBrainz takes one credit string, and the canonical ids
/// the host knows become the recording, release and artist members of
/// `additional_info`.
pub(crate) fn raw_listen(listen: &Listen, playing_now: bool) -> lb::RawListen {
    let artist_mbids = listen
        .ids
        .iter()
        .filter(|id| id.namespace == MB_ARTIST)
        .map(|id| id.value.clone())
        .collect();

    lb::RawListen {
        listened_at: if playing_now {
            None
        } else {
            Some((listen.listened_at / 1_000) as i64)
        },
        track_metadata: lb::RawTrackMetadata {
            artist_name: listen.artists.join(", "),
            track_name: listen.title.clone(),
            release_name: listen.album.clone(),
            additional_info: lb::AdditionalInfo {
                recording_mbid: id_value(&listen.ids, MB_RECORDING).map(str::to_string),
                release_mbid: id_value(&listen.ids, MB_RELEASE).map(str::to_string),
                artist_mbids,
                duration_ms: listen.duration_ms,
                tracknumber: listen.track_number,
                submission_client: Some(SUBMISSION_CLIENT.to_string()),
                media_player: listen.client.clone(),
            },
        },
    }
}

/// Returns the feedback body for a track, when ListenBrainz can take it.
///
/// # Parameters
///
/// - `track`: the track the listener gave feedback on.
/// - `value`: the feedback.
///
/// # Returns
///
/// `Some` with the body for a listener's opinion, `None` when the track carries
/// no MusicBrainz recording id: ListenBrainz identifies feedback by recording
/// id, and a track the server only knows locally has none, so there is nothing
/// to send and the call succeeds without a request.
pub(crate) fn feedback_body(track: &Listen, value: FeedbackValue) -> Option<lb::Feedback> {
    let recording_mbid = id_value(&track.ids, MB_RECORDING)?;
    let score = match value {
        FeedbackValue::Love => 1,
        FeedbackValue::Hate => -1,
        FeedbackValue::Clear => 0,
    };

    Some(lb::Feedback {
        recording_mbid: recording_mbid.to_string(),
        score,
    })
}

/// Submits finished listens or a now-playing notification.
///
/// # Parameters
///
/// - `client`: the API client, carrying the listener's token.
/// - `listens`: the listens to submit.
/// - `playing_now`: whether this announces the start of playback.
///
/// # Returns
///
/// `Ok(())` once ListenBrainz accepted the submission.
///
/// # Errors
///
/// Returns the provider error of the request: an unauthorized error for a token
/// ListenBrainz rejects, rate-limited when the account hit its limit, and
/// unavailable when the service cannot be reached.
pub(crate) fn submit(
    client: &Client<'_>,
    listens: &[Listen],
    playing_now: bool,
) -> Result<(), ProviderError> {
    if listens.is_empty() {
        return Ok(());
    }
    client.send_json_body(
        "POST",
        "1/submit-listens",
        &submission(listens, playing_now),
    )
}

/// Sends the listener's opinion of a track.
///
/// # Parameters
///
/// - `client`: the API client, carrying the listener's token.
/// - `track`: the track the feedback is about.
/// - `value`: the feedback.
///
/// # Returns
///
/// `Ok(())` once ListenBrainz accepted the feedback, and also when the track
/// has no recording id and there is nothing to send.
///
/// # Errors
///
/// Returns the provider error of the request.
pub(crate) fn feedback(
    client: &Client<'_>,
    track: &Listen,
    value: FeedbackValue,
) -> Result<(), ProviderError> {
    let Some(body) = feedback_body(track, value) else {
        return Ok(());
    };
    client.send_json_body("POST", "1/feedback/recording-feedback", &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a listen with one MusicBrainz identifier of each kind.
    fn listen() -> Listen {
        Listen {
            title: "Nothing".to_string(),
            artists: vec!["Christian Loffler".to_string(), "Adna".to_string()],
            album: Some("Felt".to_string()),
            album_artist: Some("Christian Loffler".to_string()),
            duration_ms: Some(178_800),
            track_number: Some(3),
            ids: vec![
                IdPair {
                    namespace: "mb-recording".to_string(),
                    value: "3c525387-0f7e-47e4-a731-64ce87819717".to_string(),
                },
                IdPair {
                    namespace: "isrc".to_string(),
                    value: "DEL021600123".to_string(),
                },
                IdPair {
                    namespace: "mb-release".to_string(),
                    value: "0269f70a-ebae-4c59-87bf-f41a9b1981a0".to_string(),
                },
                IdPair {
                    namespace: "mb-artist".to_string(),
                    value: "a0d91c38-f5dc-487b-9b30-b67de1e7489e".to_string(),
                },
            ],
            listened_at: 1_793_916_318_000,
            client: Some("Hearthsound web".to_string()),
        }
    }

    /// Renders a submission as the JSON ListenBrainz receives.
    fn body(listens: &[Listen], playing_now: bool) -> serde_json::Value {
        serde_json::to_value(submission(listens, playing_now)).unwrap()
    }

    #[test]
    fn one_finished_listen_is_single() {
        let body = body(&[listen()], false);
        assert_eq!(body["listen_type"], "single");
        assert_eq!(body["payload"].as_array().unwrap().len(), 1);
        assert_eq!(body["payload"][0]["listened_at"], 1_793_916_318i64);
    }

    #[test]
    fn several_finished_listens_are_import() {
        let body = body(&[listen(), listen()], false);
        assert_eq!(body["listen_type"], "import");
        assert_eq!(body["payload"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn now_playing_omits_the_timestamp() {
        let body = body(&[listen()], true);
        assert_eq!(body["listen_type"], "playing_now");
        assert!(body["payload"][0].get("listened_at").is_none());
        assert_eq!(
            body["payload"][0]["track_metadata"]["track_name"],
            "Nothing"
        );
    }

    #[test]
    fn the_track_metadata_carries_the_ids_and_the_player() {
        let body = body(&[listen()], false);
        let metadata = &body["payload"][0]["track_metadata"];
        assert_eq!(metadata["artist_name"], "Christian Loffler, Adna");
        assert_eq!(metadata["release_name"], "Felt");
        assert_eq!(
            metadata["additional_info"]["recording_mbid"],
            "3c525387-0f7e-47e4-a731-64ce87819717"
        );
        assert_eq!(
            metadata["additional_info"]["release_mbid"],
            "0269f70a-ebae-4c59-87bf-f41a9b1981a0"
        );
        assert_eq!(
            metadata["additional_info"]["artist_mbids"],
            serde_json::json!(["a0d91c38-f5dc-487b-9b30-b67de1e7489e"])
        );
        assert_eq!(metadata["additional_info"]["duration_ms"], 178_800);
        assert_eq!(metadata["additional_info"]["tracknumber"], 3);
        assert_eq!(
            metadata["additional_info"]["submission_client"],
            "Hearthsound"
        );
        assert_eq!(
            metadata["additional_info"]["media_player"],
            "Hearthsound web"
        );
    }

    #[test]
    fn a_listen_without_optional_fields_leaves_them_out() {
        let mut listen = listen();
        listen.album = None;
        listen.album_artist = None;
        listen.duration_ms = None;
        listen.track_number = None;
        listen.client = None;
        listen.ids.clear();

        let body = body(&[listen], false);
        let metadata = &body["payload"][0]["track_metadata"];
        assert!(metadata.get("release_name").is_none());
        let additional = metadata["additional_info"].as_object().unwrap();
        assert_eq!(additional.len(), 1);
        assert_eq!(additional["submission_client"], "Hearthsound");
    }

    #[test]
    fn feedback_scores_love_hate_and_clear() {
        let track = listen();
        assert_eq!(feedback_body(&track, FeedbackValue::Love).unwrap().score, 1);
        assert_eq!(
            feedback_body(&track, FeedbackValue::Hate).unwrap().score,
            -1
        );
        assert_eq!(
            feedback_body(&track, FeedbackValue::Clear).unwrap().score,
            0
        );
        assert_eq!(
            feedback_body(&track, FeedbackValue::Love)
                .unwrap()
                .recording_mbid,
            "3c525387-0f7e-47e4-a731-64ce87819717"
        );
    }

    #[test]
    fn feedback_is_skipped_without_a_recording_id() {
        let mut track = listen();
        track.ids.retain(|id| id.namespace != "mb-recording");
        assert!(feedback_body(&track, FeedbackValue::Love).is_none());
    }
}
