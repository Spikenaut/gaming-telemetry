# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Changed

- **Relicensed from GPL-3.0 to `MIT OR Apache-2.0`**
  ([#19](https://github.com/rmems/gaming-telemetry/issues/19),
  [#23](https://github.com/rmems/gaming-telemetry/pull/23))

  The root `LICENSE` (GPL-3.0) was replaced by [`LICENSE-MIT`](LICENSE-MIT) and
  [`LICENSE-APACHE`](LICENSE-APACHE); `Cargo.toml`'s `license` field now reads
  `MIT OR Apache-2.0`; every source file carries an
  `SPDX-License-Identifier: MIT OR Apache-2.0` header.

  This aligns the collector with the permissive terms of its downstream consumers
  (`corinth-canal`, `Spikenaut-SNN`, `LiquidCortex.jl`). No code semantics changed.

  **If you require GPL-3.0 terms**, pin to commit `ea6dc6d` or earlier — everything
  through that commit remains available under GPL-3.0.

### Added

- `session_label` on every Parquet row, set via the `SESSION_LABEL` environment
  variable, so multi-title capture sessions can be separated downstream
  ([#20](https://github.com/rmems/gaming-telemetry/issues/20),
  [#21](https://github.com/rmems/gaming-telemetry/pull/21))

### Removed

- The `verify_cyberpunk` workload verifier, its CI job, and verify-centric docs. The
  collector is game-agnostic: no game-install discovery, no Steam/Proton scanning, no
  mod scanning ([#21](https://github.com/rmems/gaming-telemetry/pull/21))
