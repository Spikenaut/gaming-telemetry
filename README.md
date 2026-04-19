# Gaming Telemetry: Neuromorphic Data Collector for SNN Training

## Overview
This high-performance Rust daemon is designed to capture high-fidelity GPU telemetry data from demanding gaming workloads. Specifically optimized for systems running **Resident Evil 4** and **Cyberpunk 2077** with **Path Tracing** and **DLSS 4.0**, it provides the rich, high-frequency time-series data required to train **Spiking Neural Networks (SNNs)** and Liquid State Machines.

The collector identifies "excitatory" spikes (e.g., PCIe bus floods during asset loading) and "inhibitory" signals (e.g., thermal throttling or power caps), mimicking the dynamics of biological neural systems.

## Key Features
- **Ultra-Low Latency Polling**: Captures metrics at **5-millisecond intervals** using the NVIDIA Management Library (NVML).
- **Asynchronous I/O**: To prevent performance drops during heavy gaming (Path Tracing), data is buffered in memory and written to versioned **Parquet** files (`gpu_telemetry_v1_batch_N.parquet`) asynchronously using `tokio` and `polars`.
- **DuckDB Integration**: Includes a built-in query utility for instant analysis of the captured Parquet batches.
- **Rich Metric Suite**: Captures complex hardware states beyond simple temperature and power.

## Captured Metrics
The telemetry captures a blend of fast-moving transients and slow-moving momentum metrics:
- **PCIe Rx/Tx Throughput**: Detects data floods from the CPU/Memory (e.g., BVH structure updates for Path Tracing).
- **Power Usage & Temperature**: High-frequency transients.
- **Graphics & Memory Clocks**: Tracking the "firing rate" of the silicon.
- **Throttle Reasons**: Captures bitmasks for Power, Thermal, and Sync limits (Inhibitory signals).
- **Fan Speed (RPM)**: A slow-moving physical momentum metric.
- **VRAM Utilization**: Tracks spatial memory pressure and allocation spikes.

## Prerequisites
- **OS**: Fedora 43 Linux
- **GPU**: NVIDIA (Optimized for RTX 50-series, compatible with others)
- **Drivers**: Proprietary NVIDIA drivers with NVML support.
- **Build Tools**: Rust (Cargo)

## Usage

### 1. Start the Telemetry Daemon
Run the daemon in release mode to ensure minimal overhead and maximum timing accuracy.
```bash
cargo run --release --bin gaming-telemetry
```
The daemon continuously polls telemetry and writes versioned batches as:

`gpu_telemetry_v1_batch_N.parquet`

CPU package power is recorded from the `CpuMonitor` time-delta energy-counter path.

### 2. Export Canonical CSV for `corinth-canal`
Convert one v1 Parquet batch into the stable 5-column replay schema:
```bash
cargo run --bin export_csv gpu_telemetry_v1_batch_1.parquet canonical.csv
```

Canonical CSV header (exact order):

`timestamp_ms,gpu_temp_c,gpu_power_w,cpu_tctl_c,cpu_package_power_w`

`gpu_power_w` is exported as `power_usage_mw / 1000.0`. CPU columns come from recorded parquet columns.

### 3. Optional: Analyze Data with DuckDB
Use the query utility for ad-hoc analysis:
```bash
cargo run --bin query gpu_telemetry_v1_batch_1.parquet
```

### 4. Verify The Cyberpunk Workload
Run the read-only verifier when you need a stable sidecar describing whether the target research workload is actually active:
```bash
cargo run --bin verify_cyberpunk -- [--telemetry PATH] [--out PATH] [--format text|json] [--runtime-thresholds JSON] [--debug] [--dry-run]
```

Search order:
- `~/.local/share/Steam/steamapps/common/Cyberpunk 2077`
- `~/.steam/steam/steamapps/common/Cyberpunk 2077`
- Proton prefix `~/.local/share/Steam/steamapps/compatdata/1091500/pfx/drive_c/users/steamuser/`
- Flatpak `~/.var/app/com.valvesoftware.Steam/`

Exit codes:
- `0`: verifier completed with `pass` or `warning`
- `1`: verifier completed with `error`
- `2`: usage, discovery, or unreadable input/config error
- `3`: runtime telemetry parse error

Examples:
```bash
cargo run --bin verify_cyberpunk --
```

```bash
cargo run --bin verify_cyberpunk -- --format text
```

```bash
cargo run --bin verify_cyberpunk -- \
  --telemetry tests/fixtures/runtime/pass.csv \
  --runtime-thresholds '{"avg_gpu_w":210,"max_gpu_w":290,"max_vram_mb":12000,"max_pcie_rx_mb_s":900,"avg_cpu_w":100}'
```

The verifier never modifies game files. `--dry-run` enforces stdout-only output and keeps the command strictly read-only.

## Replay Contract

One-way flow:

`collector -> gpu_telemetry_v1_batch_N.parquet -> export_csv -> canonical.csv -> corinth-canal/examples/csv_replay`

Consumer command in `corinth-canal`:
```bash
cargo run --example csv_replay canonical.csv
```

## Architecture for SNNs
The data collected is structured to be directly useful for Neuromorphic computing:
- **Excitatory Inputs**: PCIe throughput and VRAM allocation rate.
- **Firing Rates**: Clock speeds and Power transients.
- **Inhibitory Inputs**: Thermal/Power throttling bitmasks.
- **State/Momentum**: Fan speeds and absolute VRAM usage.

## License
GPL-3.0 License. See [LICENSE](LICENSE) for details.
