//! Discovery: similar tracks and artists, top tracks, recommendations and
//! generated playlists.
//!
//! The labs similarity datasets answer on their own host, so those requests go
//! to [`crate::url::LABS_BASE_URL`]; everything else uses the configured API
//! base URL. Every translation is a pure function over the decoded payloads, so
//! the shapes are unit-tested on the host target.

use hearthsound_plugin::scrobble_discovery_link_provider::exports::hearthsound::plugin::discovery::{
    ArtistSeed, GeneratedPlaylist, PlaylistSummary, SuggestedArtist, SuggestedTrack, TrackSeed,
};
use hearthsound_plugin::types::{IdPair, ProviderError};

use crate::api::{Client, METADATA_BATCH};
use crate::lb;
use crate::time;
use crate::url;

/// Name of the request member that selects metadata portions.
const METADATA_INC: &str = "artist release";

/// Most recommendations one call may ask ListenBrainz for.
///
/// The endpoint publishes a per-request ceiling; the metadata lookups that name
/// the recommendations are batched separately.
const MAX_RECOMMENDATIONS: u32 = 100;

/// Most playlists read from a user before the plugin stops.
const MAX_PLAYLISTS: usize = 100;

/// Returns the first identifier of `ids` in the given namespace.
///
/// # Parameters
///
/// - `ids`: the identifiers to search.
/// - `namespace`: the namespace to match.
///
/// # Returns
///
/// The first matching value, or `None`.
fn id_value<'a>(ids: &'a [IdPair], namespace: &str) -> Option<&'a str> {
    ids.iter()
        .find(|id| id.namespace == namespace)
        .map(|id| id.value.as_str())
}

/// Builds an id pair.
///
/// # Parameters
///
/// - `namespace`: the identifier namespace.
/// - `value`: the identifier value.
///
/// # Returns
///
/// The pair.
fn pair(namespace: &str, value: &str) -> IdPair {
    IdPair {
        namespace: namespace.to_string(),
        value: value.to_string(),
    }
}

/// Asks the labs similarity dataset for artists similar to a seed.
///
/// # Parameters
///
/// - `seed`: the seed artist.
/// - `limit`: maximum number of artists to return.
///
/// # Returns
///
/// The similar artists in the dataset's order. An empty list means the seed
/// carries no MusicBrainz artist id or the dataset knows nothing similar.
///
/// # Errors
///
/// Returns the provider error of the request.
pub(crate) fn similar_artists(
    seed: &ArtistSeed,
    limit: u32,
) -> Result<Vec<SuggestedArtist>, ProviderError> {
    let Some(mbid) = id_value(&seed.ids, "mb-artist") else {
        return Ok(Vec::new());
    };

    let path = format!(
        "similar-artists/json?artist_mbids={}&algorithm={}",
        url::encode_component(mbid),
        url::encode_component(url::SIMILAR_ARTISTS_ALGORITHM)
    );
    let rows: Vec<lb::SimilarArtist> =
        crate::api::get_absolute(&url::endpoint(url::LABS_BASE_URL, &path))?;

    Ok(suggested_artists(&rows, limit))
}

/// Asks the labs similarity dataset for tracks similar to a seed.
///
/// # Parameters
///
/// - `seed`: the seed track.
/// - `limit`: maximum number of tracks to return.
///
/// # Returns
///
/// The similar tracks in the dataset's order. An empty list means the seed
/// carries no MusicBrainz recording id or the dataset knows nothing similar.
///
/// # Errors
///
/// Returns the provider error of the request.
pub(crate) fn similar_tracks(
    seed: &TrackSeed,
    limit: u32,
) -> Result<Vec<SuggestedTrack>, ProviderError> {
    let Some(mbid) = id_value(&seed.ids, "mb-recording") else {
        return Ok(Vec::new());
    };

    let path = format!(
        "similar-recordings/json?recording_mbids={}&algorithm={}",
        url::encode_component(mbid),
        url::encode_component(url::SIMILAR_RECORDINGS_ALGORITHM)
    );
    let rows: Vec<lb::SimilarRecording> =
        crate::api::get_absolute(&url::endpoint(url::LABS_BASE_URL, &path))?;

    Ok(suggested_tracks_from_similar(&rows, limit))
}

/// Asks ListenBrainz for an artist's most listened recordings.
///
/// # Parameters
///
/// - `client`: the API client.
/// - `seed`: the seed artist.
/// - `limit`: maximum number of tracks to return.
///
/// # Returns
///
/// The artist's top recordings by listen count. An empty list means the seed
/// carries no MusicBrainz artist id.
///
/// # Errors
///
/// Returns the provider error of the request.
pub(crate) fn artist_top_tracks(
    client: &Client<'_>,
    seed: &ArtistSeed,
    limit: u32,
) -> Result<Vec<SuggestedTrack>, ProviderError> {
    let Some(mbid) = id_value(&seed.ids, "mb-artist") else {
        return Ok(Vec::new());
    };

    let rows: Vec<lb::PopularRecording> = client.get(&format!(
        "1/popularity/top-recordings-for-artist/{}",
        url::encode_component(mbid)
    ))?;

    Ok(suggested_tracks_from_popular(&rows, limit))
}

/// Reads the user's collaborative-filtering recommendations, with names.
///
/// # Parameters
///
/// - `client`: the API client, carrying the listener's token.
/// - `user_name`: the listener's ListenBrainz name.
/// - `limit`: maximum number of tracks to return.
///
/// # Returns
///
/// The recommended tracks in recommendation order. Recommendations without
/// metadata on ListenBrainz are dropped because a suggestion without a title
/// cannot be resolved.
///
/// # Errors
///
/// Returns the provider error of the request.
pub(crate) fn recommended_tracks(
    client: &Client<'_>,
    user_name: &str,
    limit: u32,
) -> Result<Vec<SuggestedTrack>, ProviderError> {
    let count = limit.clamp(1, MAX_RECOMMENDATIONS);
    let path = format!(
        "1/cf/recommendation/user/{}/recording?count={count}",
        url::encode_component(user_name)
    );

    let Some(answer) = client.get_optional::<lb::Recommendation>(&path)? else {
        return Ok(Vec::new());
    };

    let mut metadata = lb::Metadata::new();
    for batch in answer.payload.mbids.chunks(METADATA_BATCH) {
        let mbids: Vec<&str> = batch
            .iter()
            .map(|row| row.recording_mbid.as_str())
            .collect();
        let body = serde_json::json!({
            "recording_mbids": mbids,
            "inc": METADATA_INC,
        });
        let part: lb::Metadata = client.post("1/metadata/recording/", &body)?;
        metadata.extend(part);
    }

    Ok(suggested_tracks_from_metadata(
        &answer.payload.mbids,
        &metadata,
        limit,
    ))
}

/// Reads the playlists ListenBrainz generated for a user.
///
/// # Parameters
///
/// - `client`: the API client.
/// - `user_name`: the listener's ListenBrainz name.
///
/// # Returns
///
/// One generated playlist per created-for playlist, with its tracks read from
/// the playlist document. A playlist without a title or an identifier is
/// skipped.
///
/// # Errors
///
/// Returns the provider error of a request.
pub(crate) fn playlists(
    client: &Client<'_>,
    user_name: &str,
) -> Result<Vec<GeneratedPlaylist>, ProviderError> {
    let path = format!(
        "1/user/{}/playlists/createdfor?count={MAX_PLAYLISTS}",
        url::encode_component(user_name)
    );
    let created: lb::CreatedFor = client.get(&path)?;

    let mut playlists = Vec::new();
    for entry in created.playlists {
        let Some(summary) = playlist_summary(&entry.playlist) else {
            continue;
        };
        let Some(identifier) = entry.playlist.identifier.as_deref() else {
            continue;
        };
        let Some(mbid) = mbid_from_identifier(identifier) else {
            continue;
        };

        let document: lb::JspfDocument =
            client.get(&format!("1/playlist/{}", url::encode_component(mbid)))?;
        playlists.push(GeneratedPlaylist {
            summary,
            tracks: jspf_tracks(&document.playlist),
        });
    }

    Ok(playlists)
}

/// Maps labs similarity rows to suggested artists.
///
/// # Parameters
///
/// - `rows`: the rows the dataset answered.
/// - `limit`: maximum number of artists.
///
/// # Returns
///
/// One suggestion per row that names an artist and an artist id, truncated to
/// `limit`.
pub(crate) fn suggested_artists(rows: &[lb::SimilarArtist], limit: u32) -> Vec<SuggestedArtist> {
    rows.iter()
        .filter_map(|row| {
            let name = row.name.as_deref().filter(|name| !name.is_empty())?;
            let mbid = row.artist_mbid.as_deref()?;
            Some(SuggestedArtist {
                name: name.to_string(),
                ids: vec![pair("mb-artist", mbid)],
                score: row.score.map(|score| score as f32),
            })
        })
        .take(limit as usize)
        .collect()
}

/// Maps labs similarity rows to suggested tracks.
///
/// # Parameters
///
/// - `rows`: the rows the dataset answered.
/// - `limit`: maximum number of tracks.
///
/// # Returns
///
/// One suggestion per row that carries a title, a recording id and an artist
/// credit, truncated to `limit`.
pub(crate) fn suggested_tracks_from_similar(
    rows: &[lb::SimilarRecording],
    limit: u32,
) -> Vec<SuggestedTrack> {
    rows.iter()
        .filter_map(|row| {
            let title = row.recording_name.as_deref()?;
            let mbid = row.recording_mbid.as_deref()?;
            let artist = row
                .artist_credit_name
                .as_deref()
                .filter(|name| !name.is_empty())?;

            let mut ids = vec![pair("mb-recording", mbid)];
            for artist_mbid in row.artist_credit_mbids.iter().flatten() {
                ids.push(pair("mb-artist", artist_mbid));
            }

            Some(SuggestedTrack {
                title: title.to_string(),
                artists: vec![artist.to_string()],
                album: row.release_name.clone(),
                duration_ms: None,
                ids,
                score: row.score.map(|score| score as f32),
            })
        })
        .take(limit as usize)
        .collect()
}

/// Maps popularity rows to suggested tracks.
///
/// # Parameters
///
/// - `rows`: the rows the popularity endpoint answered.
/// - `limit`: maximum number of tracks.
///
/// # Returns
///
/// One suggestion per row that carries a title, a recording id and an artist,
/// truncated to `limit`.
pub(crate) fn suggested_tracks_from_popular(
    rows: &[lb::PopularRecording],
    limit: u32,
) -> Vec<SuggestedTrack> {
    rows.iter()
        .filter_map(|row| {
            let title = row.recording_name.as_deref()?;
            let mbid = row.recording_mbid.as_deref()?;
            let artist = row.artist_name.as_deref().filter(|name| !name.is_empty())?;
            Some(SuggestedTrack {
                title: title.to_string(),
                artists: vec![artist.to_string()],
                album: row.release_name.clone(),
                duration_ms: row.length,
                ids: vec![pair("mb-recording", mbid)],
                score: None,
            })
        })
        .take(limit as usize)
        .collect()
}

/// Joins recommendations with the metadata fetched for them.
///
/// # Parameters
///
/// - `recommendations`: the recommended recordings, best first.
/// - `metadata`: the metadata lookup answer, keyed by recording mbid.
/// - `limit`: maximum number of tracks.
///
/// # Returns
///
/// One suggestion per recommendation that has a title and an artist credit,
/// keeping the recommendation order and its score. The credited artist names
/// become the artist list, and each credited MusicBrainz id becomes an
/// `mb-artist` pair.
pub(crate) fn suggested_tracks_from_metadata(
    recommendations: &[lb::ScoredMbid],
    metadata: &lb::Metadata,
    limit: u32,
) -> Vec<SuggestedTrack> {
    recommendations
        .iter()
        .filter_map(|recommendation| {
            let entry = metadata.get(&recommendation.recording_mbid)?;
            let recording = entry.recording.as_ref()?;
            let title = recording.name.as_deref()?;

            let mut artists = Vec::new();
            let mut ids = vec![pair("mb-recording", &recommendation.recording_mbid)];

            if let Some(artist) = entry.artist.as_ref() {
                for credited in &artist.artists {
                    if let Some(name) = credited.name.as_deref().filter(|name| !name.is_empty()) {
                        artists.push(name.to_string());
                    }
                    if let Some(mbid) = credited.artist_mbid.as_deref() {
                        ids.push(pair("mb-artist", mbid));
                    }
                }
                if artists.is_empty()
                    && let Some(name) = artist.name.as_deref().filter(|name| !name.is_empty())
                {
                    artists.push(name.to_string());
                }
            }

            if artists.is_empty() {
                return None;
            }

            Some(SuggestedTrack {
                title: title.to_string(),
                artists,
                album: entry
                    .release
                    .as_ref()
                    .and_then(|release| release.name.clone()),
                duration_ms: recording.length,
                ids,
                score: recommendation.score.map(|score| score as f32),
            })
        })
        .take(limit as usize)
        .collect()
}

/// Derives the stable reference of a generated playlist.
///
/// # Parameters
///
/// - `playlist`: the playlist metadata.
///
/// # Returns
///
/// The generator name ListenBrainz records for the playlist slot (for example
/// `weekly-jams`), which stays the same across refreshes; the playlist's
/// identifier is used when the generator did not name itself, which keeps a
/// hand-written or third-party playlist addressable too.
fn playlist_reference(playlist: &lb::PlaylistMetadata) -> Option<String> {
    let generator = playlist
        .extension
        .as_ref()
        .and_then(|extension| extension.playlist.as_ref())
        .and_then(|body| body.additional_metadata.as_ref())
        .and_then(|metadata| metadata.algorithm_metadata.as_ref())
        .and_then(|algorithm| algorithm.source_patch.as_deref())
        .filter(|patch| !patch.is_empty());

    if let Some(patch) = generator {
        return Some(patch.to_string());
    }

    let identifier = playlist.identifier.as_deref()?;
    mbid_from_identifier(identifier).map(str::to_string)
}

/// Returns when a generated playlist retires.
///
/// # Parameters
///
/// - `playlist`: the playlist metadata.
///
/// # Returns
///
/// The expiry in unix milliseconds, when the generator records one.
fn playlist_expiry(playlist: &lb::PlaylistMetadata) -> Option<u64> {
    let value = playlist
        .extension
        .as_ref()
        .and_then(|extension| extension.playlist.as_ref())
        .and_then(|body| body.additional_metadata.as_ref())
        .and_then(|metadata| metadata.expires_at.as_deref())?;

    time::parse_iso8601_ms(value)
}

/// Builds the summary of a generated playlist.
///
/// # Parameters
///
/// - `playlist`: the playlist metadata.
///
/// # Returns
///
/// The summary, or `None` when the playlist has no title or no stable
/// reference.
pub(crate) fn playlist_summary(playlist: &lb::PlaylistMetadata) -> Option<PlaylistSummary> {
    Some(PlaylistSummary {
        reference: playlist_reference(playlist)?,
        name: playlist.title.clone()?,
        description: playlist.annotation.clone(),
        expires_at: playlist_expiry(playlist),
    })
}

/// Maps the tracks of a JSPF playlist.
///
/// # Parameters
///
/// - `playlist`: the playlist document.
///
/// # Returns
///
/// One suggestion per track in playlist order. A track without a title, an
/// artist credit and a recording identifier is dropped: the server resolves
/// suggestions by recording id, and the rest cannot identify it.
pub(crate) fn jspf_tracks(playlist: &lb::JspfPlaylist) -> Vec<SuggestedTrack> {
    playlist
        .track
        .iter()
        .filter_map(|track| {
            let title = track.title.as_deref()?;
            let identifiers = track.identifier.clone();
            let mbid = identifiers.iter().find_map(|uri| recording_mbid(uri))?;

            let mut ids = vec![pair("mb-recording", mbid)];
            let mut artists = Vec::new();

            if let Some(additional) = track
                .extension
                .as_ref()
                .and_then(|extension| extension.track.as_ref())
                .and_then(|body| body.additional_metadata.as_ref())
            {
                for credited in &additional.artists {
                    if let Some(name) = credited
                        .artist_credit_name
                        .as_deref()
                        .filter(|name| !name.is_empty())
                    {
                        artists.push(name.to_string());
                    }
                    if let Some(artist_mbid) = credited.artist_mbid.as_deref() {
                        ids.push(pair("mb-artist", artist_mbid));
                    }
                }
            }

            if artists.is_empty()
                && let Some(creator) = track.creator.as_deref().filter(|name| !name.is_empty())
            {
                artists.push(creator.to_string());
            }

            if artists.is_empty() {
                return None;
            }

            Some(SuggestedTrack {
                title: title.to_string(),
                artists,
                album: track.album.clone(),
                duration_ms: track.duration,
                ids,
                score: None,
            })
        })
        .collect()
}

/// Returns the recording id at the end of a MusicBrainz recording URI.
///
/// # Parameters
///
/// - `uri`: the identifier element of a JSPF track.
///
/// # Returns
///
/// The id that follows `/recording/`, without a query or fragment, or `None`
/// for a URI that does not name a recording.
pub(crate) fn recording_mbid(uri: &str) -> Option<&str> {
    let path = uri.split(['?', '#']).next().unwrap_or(uri);
    let index = path.rfind("/recording/")?;
    let mbid = &path[index + "/recording/".len()..];
    (!mbid.is_empty()).then_some(mbid)
}

/// Returns the last path segment of a playlist identifier.
///
/// # Parameters
///
/// - `identifier`: the playlist's canonical URL.
///
/// # Returns
///
/// The identifier's last non-empty segment, or `None` for a URL without one.
pub(crate) fn mbid_from_identifier(identifier: &str) -> Option<&str> {
    let path = identifier.split(['?', '#']).next().unwrap_or(identifier);
    let segment = path.trim_end_matches('/').rsplit('/').next()?;
    (!segment.is_empty()).then_some(segment)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses one embedded fixture.
    fn fixture<T: serde::de::DeserializeOwned>(name: &str) -> T {
        let text = match name {
            "similar_artists" => include_str!("fixtures/similar_artists.json"),
            "similar_recordings" => include_str!("fixtures/similar_recordings.json"),
            "top_recordings" => include_str!("fixtures/top_recordings.json"),
            "recommendation" => include_str!("fixtures/recommendation.json"),
            "metadata_recording" => include_str!("fixtures/metadata_recording.json"),
            "createdfor" => include_str!("fixtures/createdfor.json"),
            "playlist" => include_str!("fixtures/playlist.json"),
            other => panic!("unknown fixture {other}"),
        };
        serde_json::from_str(text).unwrap()
    }

    /// A client that is never asked to send anything.
    fn unqueried_client() -> Client<'static> {
        Client {
            base_url: "https://api.listenbrainz.org",
            token: None,
        }
    }

    #[test]
    fn a_seed_without_ids_answers_nothing() {
        let client = unqueried_client();
        let artist = ArtistSeed {
            name: "Radiohead".to_string(),
            ids: Vec::new(),
        };
        let track = TrackSeed {
            title: "Creep".to_string(),
            artists: vec!["Radiohead".to_string()],
            ids: Vec::new(),
        };

        assert!(similar_artists(&artist, 10).unwrap().is_empty());
        assert!(similar_tracks(&track, 10).unwrap().is_empty());
        assert!(artist_top_tracks(&client, &artist, 10).unwrap().is_empty());
    }

    #[test]
    fn similar_artists_keep_the_dataset_order_and_score() {
        let rows: Vec<lb::SimilarArtist> = fixture("similar_artists");
        let artists = suggested_artists(&rows, 1);

        assert_eq!(artists.len(), 1);
        assert_eq!(artists[0].name, "Nirvana");
        assert_eq!(artists[0].ids[0].namespace, "mb-artist");
        assert_eq!(
            artists[0].ids[0].value,
            "5b11f4ce-a62d-471e-81fc-a69a8278c7da"
        );
        assert_eq!(artists[0].score, Some(11_156.0));
    }

    #[test]
    fn similar_tracks_carry_the_recording_and_artist_ids() {
        let rows: Vec<lb::SimilarRecording> = fixture("similar_recordings");
        let tracks = suggested_tracks_from_similar(&rows, 10);

        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Familiar");
        assert_eq!(tracks[0].artists, vec!["Nils Frahm".to_string()]);
        assert_eq!(tracks[0].album.as_deref(), Some("Felt"));
        assert_eq!(
            tracks[0]
                .ids
                .iter()
                .map(|id| (id.namespace.as_str(), id.value.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("mb-recording", "31dad9e2-25fc-48aa-8c01-72c3729f53a1"),
                ("mb-artist", "d03a54bc-6e07-4c04-a8d4-7fbf2bd07a01"),
            ]
        );
    }

    #[test]
    fn top_tracks_read_the_duration_and_drop_untitled_rows() {
        let rows: Vec<lb::PopularRecording> = fixture("top_recordings");
        let tracks = suggested_tracks_from_popular(&rows, 10);

        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].title, "Closer");
        assert_eq!(tracks[0].artists, vec!["Nine Inch Nails".to_string()]);
        assert_eq!(tracks[0].duration_ms, Some(373_133));
        assert_eq!(tracks[1].title, "Hurt");
    }

    #[test]
    fn recommendations_take_their_names_from_the_metadata() {
        let answer: lb::Recommendation = fixture("recommendation");
        let metadata: lb::Metadata = fixture("metadata_recording");

        let tracks = suggested_tracks_from_metadata(&answer.payload.mbids, &metadata, 10);

        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Nothing");
        assert_eq!(
            tracks[0].artists,
            vec!["Christian Loffler".to_string(), "Adna".to_string()]
        );
        assert_eq!(tracks[0].album.as_deref(), Some("Felt"));
        assert_eq!(tracks[0].duration_ms, Some(178_800));
        assert_eq!(tracks[0].score, Some(9.345));
        assert_eq!(
            tracks[0]
                .ids
                .iter()
                .map(|id| id.value.as_str())
                .collect::<Vec<_>>(),
            vec![
                "3c525387-0f7e-47e4-a731-64ce87819717",
                "a0d91c38-f5dc-487b-9b30-b67de1e7489e",
                "69cc0c33-a4d9-4f48-bcd6-010eddee5fcb",
            ]
        );
    }

    #[test]
    fn recommendations_without_metadata_are_dropped() {
        let answer: lb::Recommendation = fixture("recommendation");
        let metadata = lb::Metadata::new();
        assert!(suggested_tracks_from_metadata(&answer.payload.mbids, &metadata, 10).is_empty());
    }

    #[test]
    fn the_playlist_reference_is_the_created_for_source_patch() {
        let answer: lb::CreatedFor = fixture("createdfor");
        let summary = playlist_summary(&answer.playlists[0].playlist).unwrap();

        assert_eq!(summary.reference, "weekly-jams");
        assert_eq!(summary.name, "Weekly Jams for rob");
        assert_eq!(summary.description.as_deref(), Some("A weekly playlist."));
        assert_eq!(summary.expires_at, Some(1_793_916_318_572));
    }

    #[test]
    fn the_playlist_reference_falls_back_to_the_identifier() {
        let mut playlist: lb::PlaylistMetadata = fixture::<lb::CreatedFor>("createdfor")
            .playlists
            .remove(0)
            .playlist;
        playlist.identifier = Some(
            "https://listenbrainz.org/playlist/6f1b1c0c-2aee-415e-933a-4f9b1c2d0c29".to_string(),
        );
        playlist.extension = None;

        let summary = playlist_summary(&playlist).unwrap();
        assert_eq!(summary.reference, "6f1b1c0c-2aee-415e-933a-4f9b1c2d0c29");
        assert_eq!(summary.expires_at, None);
    }

    #[test]
    fn jspf_tracks_carry_the_credit_and_the_recording_id() {
        let document: lb::JspfDocument = fixture("playlist");
        let tracks = jspf_tracks(&document.playlist);

        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[0].title, "Nothing");
        assert_eq!(
            tracks[0].artists,
            vec!["Christian Loffler".to_string(), "Adna".to_string()]
        );
        assert_eq!(tracks[0].album.as_deref(), Some("Felt"));
        assert_eq!(tracks[0].duration_ms, Some(178_800));
        assert_eq!(
            tracks[0].ids[0].value,
            "3c525387-0f7e-47e4-a731-64ce87819717"
        );
        assert_eq!(tracks[1].artists, vec!["Nils Frahm".to_string()]);
    }

    #[test]
    fn recording_ids_are_read_from_musicbrainz_uris() {
        assert_eq!(
            recording_mbid(
                "https://musicbrainz.org/recording/3c525387-0f7e-47e4-a731-64ce87819717"
            ),
            Some("3c525387-0f7e-47e4-a731-64ce87819717")
        );
        assert_eq!(
            recording_mbid("https://musicbrainz.org/recording/abc?x=1"),
            Some("abc")
        );
        assert_eq!(recording_mbid("https://musicbrainz.org/artist/abc"), None);
        assert_eq!(recording_mbid(""), None);
    }

    #[test]
    fn identifiers_lose_their_query_and_trailing_slash() {
        assert_eq!(
            mbid_from_identifier("https://listenbrainz.org/playlist/6f1b/"),
            Some("6f1b")
        );
        assert_eq!(
            mbid_from_identifier("https://listenbrainz.org/playlist/6f1b?x=1"),
            Some("6f1b")
        );
        assert_eq!(mbid_from_identifier(""), None);
    }
}
