# Hearthsound ListenBrainz plugin

A [Hearthsound](https://github.com/hearthsound) scrobbling plugin backed by
[ListenBrainz](https://listenbrainz.org). It compiles to a WebAssembly component
that implements the `scrobble` provider interface, so a Hearthsound server can
announce the track a listener starts, submit finished listens and send the
listener's love or hate for a recording.

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

The submission shapes are built by pure functions, so their unit tests need no
WebAssembly toolchain and no network:

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

## Layout

|Path|Contents|
|---|---|
|`Cargo.toml`|Crate metadata and the SDK dependency.|
|`src/lib.rs`|The plugin: configuration, lifecycle and the provider interfaces.|
|`src/api.rs`|The HTTP transport and its authorization header.|
|`src/lb.rs`|Serde types for the ListenBrainz request and response bodies.|
|`src/scrobble.rs`|Listens, now playing and feedback.|
|`src/user.rs`|The linked account's stored settings.|
|`src/url.rs`|Base URL validation and query encoding.|
|`manifest.json`|The plugin manifest the server reads.|

## License

Licensed under either of Apache License, Version 2.0 or MIT license at your
option.
