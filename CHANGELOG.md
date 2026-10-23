# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The repository scaffolding: the crate manifest, the toolchain pin, the dual
  MIT/Apache-2.0 license files, the CI workflow and this changelog.
- Serde types for the ListenBrainz request and response bodies, with the
  submission and feedback shapes the unit tests assert.
- The scrobble interface: now playing, finished listens submitted as `single`
  or `import`, and love/hate feedback addressed by MusicBrainz recording id.
- The instance's `baseUrl` setting, the user token setting, configuration
  validation and a health probe, plus the manifest that declares the plugin's
  layers and permissions.

[Unreleased]: https://github.com/hearthsound/plugin-listenbrainz/commits/main
