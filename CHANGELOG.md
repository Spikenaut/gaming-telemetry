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

- **Session manifest** (`session_manifest.json`, `schema_version: 1`) written into every
  capture directory: session id/label, start and end timestamps, requested poll interval,
  collector version, git commit, host GPU/driver/CPU model, and workload class
  ([#22](https://github.com/rmems/gaming-telemetry/issues/22))
- **Timing-quality statistics** in the manifest — observed inter-sample interval p50/p95/max,
  late-sample and skipped-tick counts, and explicit monotonic-vs-wall-clock basis fields, so a
  consumer can tell whether a nominal 5 ms stream actually behaved like one
  ([#22](https://github.com/rmems/gaming-telemetry/issues/22))
- **`SESSION_DIR`** environment variable: the collector creates and owns the session
  directory instead of requiring the operator to `cd` into it. Unset preserves the previous
  write-to-cwd behavior ([#22](https://github.com/rmems/gaming-telemetry/issues/22))
- **`WORKLOAD_CLASS`** environment variable, defaulting to `gaming`
- `session_label` on every Parquet row, set via the `SESSION_LABEL` environment
  variable, so multi-title capture sessions can be separated downstream
  ([#20](https://github.com/rmems/gaming-telemetry/issues/20),
  [#21](https://github.com/rmems/gaming-telemetry/pull/21))

### Fixed

- **Restarting the collector in a populated session directory no longer overwrites
  `gpu_telemetry_v2_batch_1.parquet`.** Batch numbering resumes from the highest existing
  batch, parsed numerically rather than lexicographically (`batch_10` sorted before `batch_2`).
  A restart also continues the existing session — preserving `session_id` and
  `started_at_utc` while incrementing `restart_count` — instead of starting a new one
  ([#22](https://github.com/rmems/gaming-telemetry/issues/22))

### Removed

- The `verify_cyberpunk` workload verifier, its CI job, and verify-centric docs. The
  collector is game-agnostic: no game-install discovery, no Steam/Proton scanning, no
  mod scanning ([#21](https://github.com/rmems/gaming-telemetry/pull/21))
