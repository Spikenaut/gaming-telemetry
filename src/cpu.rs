// SPDX-License-Identifier: MIT OR Apache-2.0

//! CPU telemetry via Linux k10temp (hwmon) and powercap RAPL energy counters.
//!
//! Every reading is an `Option`. A sensor that is absent, unreadable, or not yet
//! primed yields `None` — never `0.0`. A fabricated zero is indistinguishable
//! from a genuinely idle CPU once it reaches a training set, and silently
//! teaches a model that CPU power is constant.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// One tick of CPU telemetry. `None` means "not measured", never "zero".
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CpuSample {
    pub tctl_c: Option<f32>,
    pub ccd1_c: Option<f32>,
    pub ccd2_c: Option<f32>,
    pub package_power_w: Option<f32>,
}

/// Tracks the energy counter between ticks so package power can be differentiated.
pub struct CpuMonitor {
    k10temp_base_path: Option<PathBuf>,
    rapl_path: Option<PathBuf>,
    /// Counter ceiling: RAPL energy wraps to zero after this many microjoules.
    rapl_max_range_uj: Option<u64>,
    /// `None` until the first successful read — a delta needs two samples.
    last_energy: Option<(u64, Instant)>,
}

impl Default for CpuMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl CpuMonitor {
    /// Discover sensors, reporting on stderr whichever ones are unavailable.
    ///
    /// The report matters: an unreadable RAPL counter is the common case on a
    /// stock kernel, and without it an operator has no way to notice that the
    /// CPU power column of an entire capture is empty.
    pub fn new() -> Self {
        let k10temp_base_path = Self::discover_k10temp_path();
        let rapl_path = Self::discover_rapl_path();

        if k10temp_base_path.is_none() {
            eprintln!(
                "CPU temperature unavailable: no k10temp hwmon device found. \
                 Temperature columns will be empty."
            );
        }
        if rapl_path.is_none() {
            eprintln!(
                "CPU package power unavailable: no readable RAPL energy counter. \
                 Since CVE-2020-8694 `energy_uj` is typically root-only (0400); \
                 run as root or grant read access to record CPU power. \
                 The cpu_package_power_w column will be empty."
            );
        }

        let rapl_max_range_uj = rapl_path.as_deref().and_then(Self::read_max_energy_range);

        Self {
            k10temp_base_path,
            rapl_path,
            rapl_max_range_uj,
            last_energy: None,
        }
    }

    /// A monitor bound to no sensors; every reading is `None`.
    #[cfg(test)]
    fn disconnected() -> Self {
        Self {
            k10temp_base_path: None,
            rapl_path: None,
            rapl_max_range_uj: None,
            last_energy: None,
        }
    }

    /// Read every CPU sensor for this tick.
    pub fn poll(&mut self) -> CpuSample {
        CpuSample {
            tctl_c: self.read_temp("temp1_input"),
            ccd1_c: self.read_temp("temp3_input"),
            ccd2_c: self.read_temp("temp4_input"),
            package_power_w: self.read_power(),
        }
    }

    /// Read one hwmon temperature input, in degrees Celsius.
    fn read_temp(&self, input: &str) -> Option<f32> {
        let base = self.k10temp_base_path.as_ref()?;
        read_u64_file(&base.join(input)).map(|milli| milli as f32 / 1000.0)
    }

    /// Differentiate the energy counter into watts since the previous tick.
    fn read_power(&mut self) -> Option<f32> {
        let path = self.rapl_path.as_ref()?;
        let current_uj = read_u64_file(path)?;
        let now = Instant::now();

        // Replace the stored reading whether or not a delta can be produced, so a
        // transient read failure costs one sample instead of inflating the next.
        let previous = self.last_energy.replace((current_uj, now));
        let (previous_uj, previous_at) = previous?;

        power_watts(
            previous_uj,
            current_uj,
            self.rapl_max_range_uj,
            now.duration_since(previous_at).as_secs_f64(),
        )
    }

    /// Discover the k10temp hwmon directory under /sys/class/hwmon.
    fn discover_k10temp_path() -> Option<PathBuf> {
        let entries = fs::read_dir("/sys/class/hwmon").ok()?;
        for entry in entries.flatten() {
            let name_path = entry.path().join("name");
            if let Ok(name) = fs::read_to_string(&name_path)
                && name.trim() == "k10temp"
            {
                return Some(entry.path());
            }
        }
        None
    }

    /// Discover a RAPL energy counter this process can actually read.
    fn discover_rapl_path() -> Option<PathBuf> {
        const CANDIDATES: [&str; 3] = [
            "/sys/class/powercap/amd-energy:0/energy_uj",
            "/sys/class/powercap/intel-rapl:0/energy_uj",
            "/sys/class/powercap/intel-rapl/intel-rapl:0/energy_uj",
        ];

        // `Path::exists` is not enough. `energy_uj` is commonly present but
        // root-only, and selecting a path we cannot read yields a counter frozen
        // at its initial value — which differentiates to a plausible, constant 0 W.
        CANDIDATES
            .iter()
            .map(Path::new)
            .find(|path| read_u64_file(path).is_some())
            .map(Path::to_path_buf)
    }

    /// Read the counter ceiling that sits beside an `energy_uj` file.
    fn read_max_energy_range(energy_path: &Path) -> Option<u64> {
        let max_path = energy_path.parent()?.join("max_energy_range_uj");
        read_u64_file(&max_path).filter(|max| *max > 0)
    }
}

/// Convert an energy-counter delta into average watts over the interval.
///
/// Returns `None` when the interval is unusable rather than substituting a zero:
/// a non-positive elapsed time (a repeated tick), or a counter that ran backwards
/// with no known ceiling to unwrap it against.
fn power_watts(
    previous_uj: u64,
    current_uj: u64,
    max_range_uj: Option<u64>,
    elapsed_sec: f64,
) -> Option<f32> {
    // NaN and infinity must be rejected explicitly: a plain `<= 0.0` test lets
    // NaN through, and it would propagate into the column as a fake reading.
    if !elapsed_sec.is_finite() || elapsed_sec <= 0.0 {
        return None;
    }

    let delta_uj = if current_uj >= previous_uj {
        current_uj - previous_uj
    } else {
        // RAPL counters wrap at `max_energy_range_uj`, which on a typical desktop
        // is ~65 kJ — roughly every 11 minutes at 100 W, so this is the ordinary
        // case during a long capture, not an anomaly.
        let max = max_range_uj?;
        if previous_uj > max {
            return None;
        }
        (max - previous_uj).checked_add(current_uj)?
    };

    let watts = (delta_uj as f64 / 1_000_000.0) / elapsed_sec;
    watts.is_finite().then_some(watts as f32)
}

/// Read a file holding a single unsigned integer.
fn read_u64_file(path: &Path) -> Option<u64> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| text.trim().parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::{CpuMonitor, power_watts};

    const MAX_RANGE: u64 = 65_532_610_987;

    #[test]
    fn steady_counter_yields_average_watts() {
        // 1 J over 1 s is 1 W.
        assert_eq!(power_watts(0, 1_000_000, Some(MAX_RANGE), 1.0), Some(1.0));
        // 0.5 J over 0.005 s (one poll tick) is 100 W.
        assert_eq!(
            power_watts(1_000_000, 1_500_000, Some(MAX_RANGE), 0.005),
            Some(100.0)
        );
    }

    /// The counter wraps roughly every 11 minutes at 100 W, so a wrapped tick
    /// must produce real power, not the 0.0 W the old code emitted.
    #[test]
    fn wrapped_counter_unwraps_against_the_ceiling() {
        let previous = MAX_RANGE - 400_000;
        let current = 100_000;
        let watts = power_watts(previous, current, Some(MAX_RANGE), 0.005).expect("wrapped power");
        // 400_000 + 100_000 uJ = 0.5 J over 5 ms = 100 W.
        assert!((watts - 100.0).abs() < 0.01, "got {watts}");
    }

    #[test]
    fn counter_going_backwards_without_a_ceiling_is_unmeasurable() {
        assert_eq!(power_watts(1_000_000, 1, None, 0.005), None);
    }

    #[test]
    fn a_previous_reading_above_the_ceiling_is_unmeasurable() {
        assert_eq!(power_watts(MAX_RANGE + 1, 1, Some(MAX_RANGE), 0.005), None);
    }

    #[test]
    fn non_positive_elapsed_time_is_unmeasurable() {
        assert_eq!(power_watts(0, 1_000_000, Some(MAX_RANGE), 0.0), None);
        assert_eq!(power_watts(0, 1_000_000, Some(MAX_RANGE), -1.0), None);
        assert_eq!(power_watts(0, 1_000_000, Some(MAX_RANGE), f64::NAN), None);
    }

    /// A missing sensor must never be reported as a plausible zero.
    #[test]
    fn a_monitor_without_sensors_reports_nothing_measured() {
        let mut monitor = CpuMonitor::disconnected();
        let sample = monitor.poll();
        assert_eq!(sample.tctl_c, None);
        assert_eq!(sample.ccd1_c, None);
        assert_eq!(sample.ccd2_c, None);
        assert_eq!(sample.package_power_w, None);
    }
}
