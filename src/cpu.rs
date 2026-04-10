//! CPU telemetry via Linux k10temp (hwmon) and powercap energy counters.

use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::Instant;

/// Previous energy reading for delta calculation.
static PREV_ENERGY: Mutex<Option<(u64, Instant)>> = Mutex::new(None);

/// CPU temperature and power readings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuTelemetry {
    /// Package temperature Tctl (°C).
    pub tctl_c: f32,
    /// CCD1 die temperature (°C), if available.
    pub ccd1_c: f32,
    /// CCD2 die temperature (°C), if available.
    pub ccd2_c: f32,
    /// Package power via powercap energy delta (W).
    pub package_power_w: f32,
}

impl CpuTelemetry {
    /// Read CPU telemetry from Linux sysfs (k10temp + powercap).
    ///
    /// Falls back to zeros on any read error (non-Linux, missing drivers).
    /// Power is calculated from energy delta over time.
    pub fn read() -> Self {
        let tctl_c = Self::read_k10temp("temp1_input");
        let ccd1_c = Self::read_k10temp("temp3_input");
        let ccd2_c = Self::read_k10temp("temp4_input");
        let package_power_w = Self::read_powercap_power_delta();
        Self {
            tctl_c,
            ccd1_c,
            ccd2_c,
            package_power_w,
        }
    }

    fn read_k10temp(sensor: &str) -> f32 {
        // Find hwmon path for k10temp
        let Ok(entries) = std::fs::read_dir("/sys/class/hwmon") else {
            return 0.0;
        };
        for entry in entries.flatten() {
            let name_path = entry.path().join("name");
            if let Ok(name) = std::fs::read_to_string(&name_path) {
                if name.trim() == "k10temp" {
                    let sensor_path = entry.path().join(sensor);
                    if let Ok(raw) = std::fs::read_to_string(&sensor_path) {
                        if let Ok(milli_c) = raw.trim().parse::<i64>() {
                            return milli_c as f32 / 1000.0;
                        }
                    }
                }
            }
        }
        0.0
    }

    /// Read energy counter and calculate power from delta over time.
    /// Returns power in watts.
    fn read_powercap_power_delta() -> f32 {
        let paths = [
            "/sys/class/powercap/amd-energy:0/energy_uj",
            "/sys/class/powercap/intel-rapl:0/energy_uj",
            "/sys/class/powercap/intel-rapl/intel-rapl:0/energy_uj",
        ];

        let now = Instant::now();

        for path in &paths {
            if let Ok(raw) = std::fs::read_to_string(path) {
                if let Ok(current_uj) = raw.trim().parse::<u64>() {
                    let mut guard = PREV_ENERGY.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

                    let power_w = if let Some((prev_uj, prev_time)) = *guard {
                        let delta_uj = current_uj.saturating_sub(prev_uj);
                        let delta_secs = prev_time.elapsed().as_secs_f64();

                        if delta_secs > 0.0 {
                            // Convert microjoules to watts (joules per second)
                            (delta_uj as f64 / delta_secs / 1_000_000.0) as f32
                        } else {
                            0.0
                        }
                    } else {
                        0.0 // First reading, no delta available yet
                    };

                    *guard = Some((current_uj, now));
                    return power_w.max(0.0); // Clamp negative values (counter wrap)
                }
            }
        }
        0.0
    }
}
