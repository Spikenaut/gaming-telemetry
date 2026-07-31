# Gaming Telemetry: Neuromorphic Data Collector for SNN Training

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

Use a dedicated per-session directory so batches from different runs do not overwrite each other.

```bash
# Examples: kcd2, re2r, re3r, re_requiem, cp2077, …
TS=$(date +%Y%m%d_%H%M%S)
SESSION_DIR="neuromorphic_data/kcd2_${TS}"
mkdir -p "$SESSION_DIR" && cd "$SESSION_DIR"
SESSION_LABEL=kcd2 cargo run --release --bin gaming-telemetry --manifest-path ../../Cargo.toml
# Ctrl+C to flush, then cd back for the next session
cd ../..

TS=$(date +%Y%m%d_%H%M%S)
SESSION_DIR="neuromorphic_data/re2r_${TS}"
mkdir -p "$SESSION_DIR" && cd "$SESSION_DIR"
SESSION_LABEL=re2r cargo run --release --bin gaming-telemetry --manifest-path ../../Cargo.toml
# Ctrl+C to flush, then cd back for export/query steps
cd ../..
```

Then:

1. Set the game to the highest graphics settings available.
2. Play the session while the collector runs (default poll: **5 ms**, override with `POLL_INTERVAL_MS`).
3. Ctrl+C to flush the last batch and exit.

Output files (multiple batches per directory):

`gpu_telemetry_v2_batch_N.parquet`

The export and query examples below reuse the `$SESSION_DIR` variable from the capture block you ran. If you used a different directory, substitute its name.

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
| Resident Evil Requiem | `re_requiem` |
| Cyberpunk 2077 | `cp2077` |

Max settings only. No install path is required by this repo.

## Design notes

- Collector never walks `$HOME`, Steam libraries, or Proton prefixes.
- Path redaction helpers remain for error logs / query display only.
- The old Cyberpunk **workload verifier** direction (PR #6 and residual skeleton/CI) was removed; see issue #20 / Linear RM-174.

## License

GPL-3.0. See [LICENSE](LICENSE).
