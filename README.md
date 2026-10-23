# Hearthsound ListenBrainz plugin

A [Hearthsound](https://github.com/hearthsound) scrobbling and discovery plugin backed by
[ListenBrainz](https://listenbrainz.org). It compiles to a WebAssembly component
that implements the `scrobble` and `discovery` provider interfaces and the
`account-link` capability, so a Hearthsound server can announce the track a
listener starts, submit finished listens, send the listener's love or hate for a
recording, connect their ListenBrainz account, and read suggestions from
ListenBrainz.

## Requirements

- Rust 1.99 or newer, with the `wasm32-wasip2` target. The included
  `rust-toolchain.toml` pins both; `rustup toolchain install` fetches them.

## Build

```console
cargo build --release --locked --target wasm32-wasip2
```

The component lands at
`target/wasm32-wasip2/release/hearthsound_plugin_listenbrainz.wasm`.

## Check and pack

Install the plugin CLI from the SDK repository:

```console
cargo install --locked --git https://github.com/hearthsound/plugin-sdk --tag v0.3.0 hearthsound-plugin-cli
```

Then validate the manifest and the component:

```console
hsplugin check
```

It prints `ok: app.hearthsound.listenbrainz 0.1.0 (provides scrobble,
discovery)` on success.

Finally, build the archive you install on a server:

```console
hsplugin pack --out-dir dist
```

`dist/app.hearthsound.listenbrainz-0.1.0.hsplugin` contains the manifest and the
component.

## Tests

The submission and suggestion shapes are built by pure functions, so their unit
tests run against captured response fixtures and need no WebAssembly toolchain
and no network:

```console
cargo test
```

## Configuration

|Instance setting|Default|Meaning|
|---|---|---|
|`baseUrl`|`https://api.listenbrainz.org`|Base URL of the ListenBrainz API. Point it at a mirror or a test double to keep traffic off ListenBrainz.|

The user setting is the ListenBrainz user token from
<https://listenbrainz.org/settings/>. It is marked `writeOnly`, so the server
seals it and never renders it back.

Connecting an account validates the token with ListenBrainz before it is
stored: an invalid token is refused with the service's answer, and a valid one
records the MusicBrainz user name the token belongs to so the recommendation and
playlist endpoints can address the listener. The plugin hands the config back to
the server, which seals it before storing it.

The instance requests a `host-http` rate limit of one request per second, which
keeps a submission stream below the limit ListenBrainz publishes for user
tokens.

## Notes

- A submission of one finished listen uses ListenBrainz's `single` type and a
  submission of several uses `import`; a now-playing notification uses
  `playing_now` and carries no timestamp, because ListenBrainz treats that
  member as the start of playback rather than a listen.
- ListenBrainz takes one credit string per listen. Several credited artists are
  joined with commas, and their MusicBrainz artist ids travel in
  `additional_info.artist_mbids` next to the recording and release ids the
  server knows.
- Feedback is addressed by MusicBrainz recording id. A track the server only
  knows locally has none, so the call succeeds without sending anything rather
  than failing the listener's love or hate.
- Love is submitted as score `1`, hate as `-1` and clearing it as `0`, which is
  how ListenBrainz deletes feedback.
- The plugin asks the host to batch at most 100 listens per call and to wait at
  least a second between calls.
- Similar artists and similar tracks come from the labs similarity datasets,
  which answer on `labs.api.listenbrainz.org` and identify a seed only by
  MusicBrainz id. A seed without one answers an empty list, which tells the
  server to ask the next instance rather than to show nothing.
- An artist's top recordings come from the popularity endpoint, so they are the
  recordings ListenBrainz counts the most listens for, in its order.
- Recommendations are the collaborative-filtering recordings for the linked
  user. The endpoint returns only ids, so their titles, credits and durations
  are read from the metadata endpoint; a recommendation without metadata is
  dropped because a suggestion without a title cannot be resolved.
- Generated playlists come from the created-for listing. Each playlist's
  reference is the generator ListenBrainz records for the slot, for example
  `weekly-jams`, so a refresh replaces the playlist in place; its expiry is read
  from the generator's metadata when present.
- Similarity and recommendation scores are passed through as the service reports
  them, so the server can order or display them without the plugin rescaling
  values whose meaning is the service's.
- ListenBrainz has no browser authorization flow for third-party applications,
  so the plugin links with the user-token method only. The two browser methods
  answer with an explanatory error rather than pretending a flow exists.

## Layout

|Path|Contents|
|---|---|
|`Cargo.toml`|Crate metadata and the SDK dependency.|
|`src/lib.rs`|The plugin: configuration, lifecycle and the provider interfaces.|
|`src/api.rs`|The HTTP transport and its authorization header.|
|`src/lb.rs`|Serde types for the ListenBrainz request and response bodies.|
|`src/link.rs`|Account linking: token validation and the stored account.|
|`src/discovery.rs`|Suggestions, recommendations and generated playlists.|
|`src/time.rs`|Parsing of the timestamps ListenBrainz returns.|
|`src/scrobble.rs`|Listens, now playing and feedback.|
|`src/user.rs`|The linked account's stored settings.|
|`src/url.rs`|Base URL validation and query encoding.|
|`manifest.json`|The plugin manifest the server reads.|

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your
option.
