---
license: gpl-3.0
task_categories:
- time-series-forecasting
tags:
- neuromorphic
- snn
- liquid-state-machines
- gaming
- hardware-telemetry
- gpu
pretty_name: Metis SMoE Latent Telemetry (Gaming)
---

# Metis SMoE Latent Telemetry
## Neuromorphic Hardware Telemetry from demanding Gaming Workloads

This dataset provides high-fidelity, high-frequency (5ms interval) hardware telemetry data captured from extreme PC gaming workloads. This dataset is optimized to simulate the biological responses of a nervous system to intense stimulus (excitatory input, action potentials/firing rates, and inhibitory responses).

### Context
The telemetry data was recorded using a custom Rust-based data collector via the NVIDIA Management Library (NVML) on a Fedora 43 Linux system. Workloads represent highly transient rendering applications including:
- **Resident Evil 4 (Remake)** (with rendering complexities)
- **Cyberpunk 2077** (Path Tracing, DLSS 4.0)

This system provides the rich, high-frequency time-series data required to train **Spiking Neural Networks (SNNs)** and **Liquid State Machines (LSMs)**.

### Neuromorphic Mapping (SNN Utility)
This data behaves as "sensorimotor" stimulus for neural networks:
- **Excitatory Inputs (Stimulus):** High surges in `pcie_rx_kbps` indicate asset floods (e.g., BVH structure updates for Path Tracing), mimicking sensory signals entering the system.
- **Action Potentials (Firing Rates):** `encoder_util_perc`, `decoder_util_perc`, and overall spatial `power_usage_mw` transients represent internal activity and network firing rates.
- **Inhibitory Inputs (Refractory/Limits):** Non-zero `throttle_reasons_bitmask` signals and thermal limits act as inhibitory governors, dynamically suppressing system activity.
- **State/Momentum:** Slow-moving environmental data like temperatures (`cpu_tctl_c`, `temperature_c`) and memory capacity.

### Data Schema

The data is provided natively as Parquet files partitioned into train batches (`system_telemetry_v1_batch_*.parquet`).

| Feature | Type | Description |
| :--- | :--- | :--- |
| `timestamp_ms` | `Int64` | UNIX timestamp in milliseconds (5ms interval captures). |
| `power_usage_mw` | `UInt32` | Total GPU power usage in milliwatts. |
| `temperature_c` | `Float32` | GPU core temperature in Celsius. |
| `pcie_rx_kbps` | `UInt32` | Incoming PCIe throughput in Kilobytes per second (Excitatory). |
| `pcie_tx_kbps` | `UInt32` | Outgoing PCIe throughput in Kilobytes per second. |
| `encoder_util_perc` | `Float32` | NVIDIA Encoder (NVENC) utilization percentage. |
| `decoder_util_perc` | `Float32` | NVIDIA Decoder (NVDEC) utilization percentage. |
| `mangohud_active` | `Boolean` | Whether MangoHud overlay telemetry was active during the snapshot. |
| `cpu_tctl_c` | `Float32` | Primary CPU package temperature (Tctl). |
| `cpu_ccd1_c` | `Float32` | Temperature of CPU Core Complex Die 1. |
| `cpu_ccd2_c` | `Float32` | Temperature of CPU Core Complex Die 2. |
| `throttle_reasons_bitmask`| `UInt64` | Bitmask defining hardware throttling events (Power, Thermal, Sync) - acts as Inhibitory signals. |

### Usage with Hugging Face `datasets`

You can seamlessly integrate this telemetry into your Neuromorphic modeling workflows using the Hugging Face `datasets` library.

```python
from datasets import load_dataset
import pyarrow.parquet as pq

# Load the entire telemetry dataset as a single stream
dataset = load_dataset("rmems/Metis-SMoE-Latent-Telemetry", split="train")

print(dataset.features)
print(dataset[0])
```

### Export to Canonical CSV (For Corinth Canal Replay)
If you are using the Spikenaut `corinth-canal` framework, you can export a canonical CSV by grabbing a single dataset file:

```bash
cargo run --bin export_csv data/train/system_telemetry_v1_batch_1.parquet canonical.csv
```

### License
This dataset is distributed under the GPL-3.0 License.
