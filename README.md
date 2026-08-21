# Gaming Telemetry: Neuromorphic Data Collector for SNN Training

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

## Overview

High-frequency GPU/CPU telemetry for a workstation (optimized for **RTX 5080** under max-settings games). The collector is **game-agnostic**: it reads NVIDIA NVML + Linux CPU sensors and writes Parquet batches for neuromorphic / SNN training.

Signals map roughly to an artificial “nervous system” for models that need to learn how compute load moves a GPU:

- **Excitatory**: PCIe floods, power/clock spikes, VRAM allocation jumps
- **Inhibitory**: thermal / power throttle bitmasks
- **State / momentum**: fan speed, absolute VRAM usage

There is **no** game-install verifier, Steam/Proton discovery, or mod scanner. Graphics settings are an **operator checklist**, not code. New titles only need a new `SESSION_LABEL` and a play session.

## Captured metrics

- Power usage & temperature
- Graphics & memory clocks
- PCIe Rx/Tx throughput
- Performance state & throttle reasons
- Fan speed, VRAM used/total
- Encoder/decoder utilization
- CPU Tctl / CCD temps and package power (hwmon + RAPL energy delta)
- **`session_label`** (string; same for every row in a run)

MangoHud (or any overlay) is **not** recorded. You may still run it yourself for on-screen monitoring; the collector only writes hardware telemetry.

## Prerequisites

- **OS**: Linux (developed on Fedora)
- **GPU**: NVIDIA with NVML (RTX 50-series preferred)
- **Build**: Rust / Cargo

## Usage

### 1. Capture a labeled session

Set `SESSION_DIR` and the collector creates that directory and writes everything into it, so batches from different runs never overwrite each other. No `cd` required.

```bash
# Examples: kcd2, re2r, re3r, re_requiem, cp2077, …
SESSION_DIR="neuromorphic_data/kcd2_$(date +%Y%m%d_%H%M%S)" SESSION_LABEL=kcd2 \
  cargo run --release --bin gaming-telemetry

SESSION_DIR="neuromorphic_data/re2r_$(date +%Y%m%d_%H%M%S)" SESSION_LABEL=re2r \
  cargo run --release --bin gaming-telemetry
```

`SESSION_DIR` is optional — unset, the collector writes to the current directory as it always did.

Then:

1. Set the game to the highest graphics settings available.
2. Play the session while the collector runs (default poll: **5 ms**, override with `POLL_INTERVAL_MS`).
3. Ctrl+C to flush the last batch and exit.

Each session directory contains:

```text
neuromorphic_data/kcd2_20260816_101500/
├── session_manifest.json          # what was captured, and how well
└── gpu_telemetry_v2_batch_N.parquet
```

Restarting the collector into an existing session directory **continues** that session:
`session_id` and `started_at_utc` are preserved, `restart_count` increments, and batch
numbering resumes from the highest existing batch instead of overwriting batch 1.

The export and query examples below reuse the `$SESSION_DIR` variable from the capture block you ran. If you used a different directory, substitute its name.

### The session manifest

Written at start so the directory is self-describing during capture, then finalized on clean
shutdown with the end time and timing statistics. It is written via a temp file and rename, so a
crash mid-write can never leave truncated JSON.

```json
{
  "schema_version": 1,
  "session_id": "kcd2_20260816_101500",
  "session_label": "kcd2",
  "started_at_utc": "2026-08-16T10:15:00.123Z",
  "ended_at_utc": "2026-08-16T11:02:31.887Z",
  "poll_interval_ms_requested": 5,
  "collector_version": "0.1.0",
  "git_commit": "54f5b74",
  "restart_count": 0,
  "host": { "gpu_name": "NVIDIA GeForce RTX 5080", "driver": "580.00", "cpu_model": "…" },
  "workload": { "class": "gaming", "label": "kcd2" },
  "parquet_write_failures": 0,
  "timing": {
    "poll_interval_ms_requested": 5,
    "sample_count": 1440000,
    "observed_interval_ms": { "p50": 5.1, "p95": 5.4, "max": 41.2 },
    "late_sample_count": 812,
    "skipped_tick_estimate": 190,
    "elapsed_basis": "monotonic",
    "row_timestamp_basis": "wall_clock_utc"
  }
}
```

**Why the timing block matters.** A nominal 5 ms stream does not necessarily behave like one.
`observed_interval_ms` reports what actually happened, `late_sample_count` counts intervals
exceeding 1.5× the requested one, and `skipped_tick_estimate` counts ticks dropped under
`MissedTickBehavior::Skip`. The two `*_basis` fields exist because intervals are measured on the
**monotonic** clock while row `timestamp_ms` comes from the **wall** clock — an NTP step moves one
and not the other, and a consumer aligning them needs to know that.

`timing.sample_count` counts samples acquired by the collector. If
`parquet_write_failures` is nonzero, one or more acquired batches were not persisted, so consumers
must account for that data loss. `ended_at_utc` marks when collection stopped, before any remaining
batch writes are drained.

`workload.class` defaults to `gaming`; override with `WORKLOAD_CLASS`.

The manifest deliberately records no usernames, home paths, Steam identifiers, or machine
inventory — only hardware model names.

### 2. Export canonical CSV for `corinth-canal`

Stable **5-column** replay schema (unchanged; `session_label` stays in Parquet):

```bash
cargo run --bin export_csv -- "$SESSION_DIR/gpu_telemetry_v2_batch_1.parquet" canonical.csv
```

Header:

`timestamp_ms,gpu_temp_c,gpu_power_w,cpu_tctl_c,cpu_package_power_w`

`gpu_power_w` is `power_usage_mw / 1000.0`.

### 3. Optional: DuckDB query helper

```bash
cargo run --bin query -- "$SESSION_DIR/gpu_telemetry_v2_batch_1.parquet"
```

## Replay contract

```text
collector -> neuromorphic_data/<session>/gpu_telemetry_v2_batch_N.parquet -> export_csv -> canonical.csv -> corinth-canal/examples/csv_replay
```

```bash
# From the corinth-canal checkout (not this repo root):
cargo run --example csv_replay --manifest-path path/to/corinth-canal/Cargo.toml -- canonical.csv
```

For multi-title training mixes, group by Parquet `session_label` (or by folder under `neuromorphic_data/`).

## Operator checklist (not code)

| Title | Suggested `SESSION_LABEL` |
|-------|---------------------------|
| Kingdom Come Deliverance 2 | `kcd2` |
| Resident Evil 2 Remake | `re2r` |
| Resident Evil 3 Remake | `re3r` |
| Resident Evil 4 Remake | `re4r` |
| Resident Evil Requiem | `re_requiem` |
| Cyberpunk 2077 | `cp2077` |

Max settings only. No install path is required by this repo.

## Design notes

- Collector never walks `$HOME`, Steam libraries, or Proton prefixes.
- Path redaction helpers remain for error logs / query display only.
- The old Cyberpunk **workload verifier** direction (PR #6 and residual skeleton/CI) was removed; see issue #20 / Linear RM-174.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

Previous releases (up to and including commit `ea6dc6d`) were published under GPL-3.0.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
