# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.1] - 2026-10-27

### Fixed

- `artist-top-tracks` sends no request when the listener has no linked account.
  ListenBrainz began answering its popularity endpoints with `401` and "you need
  to provide an Auth token", so an anonymous call could only fail; the call now
  uses the linked account's token, and without one it answers an empty list so
  the catalog layer's other instances can answer the top-songs shelf instead.

## [0.2.0] - 2026-10-25

### Added

- The `labsBaseUrl` instance setting (default
  `https://labs.api.listenbrainz.org`), so the similarity datasets can be read
  from a labs mirror or a test double. It is validated like `baseUrl`, and a
  configured value appears in the built `similar-artists` and
  `similar-recordings` URLs.

### Changed

- Build against the `hearthsound:plugin@0.4.0` API (plugin-sdk v0.4.0).
- The labs similarity calls stay unauthenticated: the configured labs base URL
  replaces the built-in constant, and the listener's token is never sent to the
  second origin that serves the datasets.
- A refused user token now reports "ListenBrainz did not accept the user token.
  Check the token and try again." instead of the bare word `unauthorized`.

## [0.1.0] - 2026-10-23

### Added

- The repository scaffolding: the crate manifest, the toolchain pin, the dual
  MIT/Apache-2.0 license files, the CI workflow and this changelog.
- Serde types for the ListenBrainz request and response bodies, with the
  submission and feedback shapes the unit tests assert.
- The scrobble interface: now playing, finished listens submitted as `single`
  or `import`, and love/hate feedback addressed by MusicBrainz recording id.
- The discovery interface: similar artists and tracks from the labs similarity
  datasets, an artist's top recordings from the popularity endpoint,
  collaborative-filtering recommendations with their names filled from the
  metadata endpoint, and the playlists ListenBrainz generates for the user with
  their tracks read from the playlist document.
- Account linking with a user token: the plugin validates the token with
  ListenBrainz, records the MusicBrainz user name it belongs to and hands the
  config back to the server, which seals it; the scrobble and discovery calls
  read it back.
- The instance's `baseUrl` setting, the user token setting, configuration
  validation and a health probe, plus the manifest that declares the plugin's
  layers and permissions.

[Unreleased]: https://github.com/hearthsound/plugin-listenbrainz/commits/main
[0.2.0]: https://github.com/hearthsound/plugin-listenbrainz/releases/tag/v0.2.0
[0.1.0]: https://github.com/hearthsound/plugin-listenbrainz/releases/tag/v0.1.0
