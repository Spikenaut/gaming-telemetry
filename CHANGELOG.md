# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Fixed

- **The build was broken.** The dependency bump to `polars 0.55.2` changed
  `LazyFrame::scan_parquet` to take a `PlRefPath`, made `DataFrame::new` take an
  explicit height, and dropped `IntoIterator` for `&ChunkedArray`; `sentry 0.49.2`
  made `ClientOptions` `#[non_exhaustive]` and replaced `sample_rate` /
  `traces_sample_rate` with sampling-strategy enums. No call site had been updated,
  so neither the collector nor `export_csv` compiled. All call sites now match the
  pinned APIs.
- **Ctrl+C was not the only way out of the poll loop.** Exhausting the batch-ID
  namespace returned straight out of `main`, dropping the buffered samples and
  aborting in-flight Parquet writes with the `JoinSet`. Both that path and an
  unnumberable final batch now converge on the normal shutdown: drain outstanding
  writes, then finalize the manifest.
- A Parquet write task that **panicked** was counted as a failure but only printed
  to stderr, never reported. It now goes through the same redact-and-report path as
  an I/O error.

### Changed

- **`SENTRY_AUTH_TOKEN` is no longer read at runtime; use `SENTRY_DSN`.** The
  collector parsed that variable as a client DSN while
  [`sentry-release.yml`](.github/workflows/sentry-release.yml) uses the same name for
  an **org-scoped API token**. One name for two secrets of very different blast
  radius invited leaking a CI credential to every collector host. The runtime now
  reads only `SENTRY_DSN`; the release workflow is unchanged.
- **`duckdb` and `sentry` are now optional** behind the `query` and `sentry` cargo
  features, both off by default ([#20](https://github.com/rmems/gaming-telemetry/issues/20)).
  `duckdb`'s `bundled` feature compiles the whole DuckDB C++ tree and dominated build
  time and peak RAM on a workstation that is also running the game being measured;
  `sentry` pulled an HTTP/TLS stack into a local poll daemon. A default build is now
  just the collector. Build the helper with `cargo run --features query --bin query`.
- `duckdb`'s unused `polars` feature (its Arrow↔Polars bridge) was dropped —
  `query.rs` only issues plain SQL through `Connection`/`row.get`.
- Sentry bootstrap moved out of `main.rs` into `gaming_telemetry::observability`,
  which compiles to no-ops when the feature is off, so call sites carry no `#[cfg]`.
- Parquet columns are now built with the column name and its source field on one
  line, removing the 18 separate passes over the sample slice and the per-row clone
  of the run-invariant `session_label`.
- Rust edition bumped to 2024 and MSRV to 1.98.0.

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
