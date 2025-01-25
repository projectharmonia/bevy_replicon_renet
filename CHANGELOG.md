# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- `bevy_replicon_renet::client::RepliconRenetClientPlugin` now should be imported as `bevy_replicon_renet::RepliconRenetClientPlugin`.
- `bevy_replicon_renet::server::RepliconRenetServerPlugin` now should be imported as `bevy_replicon_renet::RepliconRenetServerPlugin`.

## [0.6.0] - 2024-12-25

### Changed

- Update to `bevy_replicon` 0.29 and `renet` 1.0.0.

## [0.5.1] - 2024-11-29

### Added

- Extension traits for conversion between Renet's and Replicon's `ClientId`s.

## [0.5.0] - 2024-09-04

### Changed

- Update to `bevy_replicon` 0.28.

## [0.4.0] - 2024-07-21

### Added

- `server` and `client` features to disable unneeded functionality.

### Changed

- Update to `bevy_replicon` 0.27.
- Move to a dedicated repository.
- Move `RepliconRenetServerPlugin` to `server` module.
- Move `RepliconRenetClientPlugin` to `client` module.

## [0.3.0] - 2024-05-26

### Changed

- Update to `bevy_replicon` 0.26.

### Fixed

- Properly set `RepliconClientStatus::Connecting` when `RenetClient` is connecting.

## [0.2.0] - 2024-05-11

### Changed

- Update to `bevy_replicon` 0.25.

## [0.1.0] - 2024-05-06

First release after I/O abstraction.

[unreleased]: https://github.com/projectharmonia/bevy_replicon_renet/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/projectharmonia/bevy_replicon_renet/compare/v0.5.1...v0.6.0
[0.5.1]: https://github.com/projectharmonia/bevy_replicon_renet/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/projectharmonia/bevy_replicon_renet/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/projectharmonia/bevy_replicon_renet/releases/tag/v0.4.0
[0.3.0]: https://github.com/projectharmonia/bevy_replicon/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/projectharmonia/bevy_replicon/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/projectharmonia/bevy_replicon/releases/tag/v0.1.0
