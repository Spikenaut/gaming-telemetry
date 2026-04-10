//! CPU telemetry via Linux k10temp (hwmon) and powercap energy counters.

use serde::{Deserialize, Serialize};

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
    pub fn read() -> Self {
        let tctl_c = Self::read_k10temp("temp1_input");
        let ccd1_c = Self::read_k10temp("temp3_input");
        let ccd2_c = Self::read_k10temp("temp4_input");
        let package_power_w = Self::read_powercap_power();
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

    fn read_powercap_power() -> f32 {
        // Read AMD RAPL energy from powercap (microjoules)
        let paths = [
            "/sys/class/powercap/amd-energy:0/energy_uj",
            "/sys/class/powercap/intel-rapl:0/energy_uj",
        ];
        for path in &paths {
            if let Ok(raw) = std::fs::read_to_string(path) {
                if let Ok(_uj) = raw.trim().parse::<u64>() {
                    // Would need delta measurement over time for true power
                    // Return 0 — caller should use time-delta approach
                    return 0.0;
                }
            }
        }
        0.0
    }
}
