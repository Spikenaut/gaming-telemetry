# Gaming Telemetry: Neuromorphic Data Collector for SNN Training

## Overview
This high-performance Rust daemon is designed to capture high-fidelity GPU telemetry data from demanding gaming workloads. Specifically optimized for systems running **Resident Evil 4** and **Cyberpunk 2077** with **Path Tracing** and **DLSS 4.0**, it provides the rich, high-frequency time-series data required to train **Spiking Neural Networks (SNNs)** and Liquid State Machines.

The collector identifies "excitatory" spikes (e.g., PCIe bus floods during asset loading) and "inhibitory" signals (e.g., thermal throttling or power caps), mimicking the dynamics of biological neural systems.

## Key Features
- **Ultra-Low Latency Polling**: Captures metrics at **5-millisecond intervals** using the NVIDIA Management Library (NVML).
- **Asynchronous I/O**: To prevent performance drops during heavy gaming (Path Tracing), data is buffered in memory and written to **Parquet** files asynchronously using `tokio` and `polars`.
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
cargo run --release
```
The daemon will continuously poll the GPU and save data into `gpu_telemetry_batch_N.parquet` files once the memory buffer fills.

### 2. Analyze Data with DuckDB
Use the provided query utility to analyze a specific batch. This tool identifies significant hardware events like excitatory spikes and inhibitory throttling.
```bash
cargo run --bin query gpu_telemetry_batch_1.parquet
```

## Architecture for SNNs
The data collected is structured to be directly useful for Neuromorphic computing:
- **Excitatory Inputs**: PCIe throughput and VRAM allocation rate.
- **Firing Rates**: Clock speeds and Power transients.
- **Inhibitory Inputs**: Thermal/Power throttling bitmasks.
- **State/Momentum**: Fan speeds and absolute VRAM usage.

## License
GPL-3.0 License. See [LICENSE](LICENSE) for details.