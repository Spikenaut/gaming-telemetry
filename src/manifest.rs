// SPDX-License-Identifier: MIT OR Apache-2.0

//! Versioned session manifest — the sidecar that makes a capture directory
//! self-describing.
//!
//! Written at session start so it exists during capture, and finalized on clean
//! shutdown with the end timestamp and timing-quality summary. Deliberately
//! records nothing personal: no usernames, home paths, Steam identifiers,
//! secrets, or wider machine inventory.

use crate::timing::TimingSummary;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Sidecar filename inside the session directory.
pub const MANIFEST_FILENAME: &str = "session_manifest.json";
/// Bump only on a breaking change to the field layout below.
pub const SCHEMA_VERSION: u32 = 1;

fn unknown() -> String {
    "unknown".to_owned()
}

/// Hardware identity of the capturing machine. Model names only — nothing that
/// identifies the operator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostInfo {
    pub gpu_name: String,
    pub driver: String,
    pub cpu_model: String,
}

impl HostInfo {
    /// GPU fields come from the caller's existing NVML handle; the CPU model is
    /// read here. Every field degrades to `"unknown"` — host detection must never
    /// abort a capture.
    pub fn new(gpu_name: Option<String>, driver: Option<String>) -> Self {
        Self {
            gpu_name: gpu_name.unwrap_or_else(unknown),
            driver: driver.unwrap_or_else(unknown),
            cpu_model: read_cpu_model().unwrap_or_else(unknown),
        }
    }
}

/// Parse the `model name` line out of `/proc/cpuinfo` contents.
pub fn parse_cpu_model(contents: &str) -> Option<String> {
    contents
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(key, _)| key.trim() == "model name")
        })
        .map(|(_, value)| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn read_cpu_model() -> Option<String> {
    parse_cpu_model(&std::fs::read_to_string("/proc/cpuinfo").ok()?)
}

/// What was running during the capture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workload {
    pub class: String,
    pub label: String,
}

impl Workload {
    pub fn new(label: &str) -> Self {
        let class = std::env::var("WORKLOAD_CLASS")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "gaming".to_owned());
        Self {
            class,
            label: label.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionManifest {
    pub schema_version: u32,
    pub session_id: String,
    pub session_label: String,
    pub started_at_utc: DateTime<Utc>,
    pub ended_at_utc: Option<DateTime<Utc>>,
    pub poll_interval_ms_requested: u64,
    pub collector_version: String,
    pub git_commit: String,
    /// How many collector processes have written into this directory. 0 on the
    /// first run.
    pub restart_count: u32,
    pub host: HostInfo,
    pub workload: Workload,
    /// Total Parquet batches that failed during this session. A nonzero
    /// value means timing sample_count can exceed rows available on disk.
    #[serde(default)]
    pub parquet_write_failures: u32,
    pub timing: Option<TimingSummary>,
}

fn manifest_path(dir: &Path) -> PathBuf {
    dir.join(MANIFEST_FILENAME)
}

fn temp_path(dir: &Path) -> PathBuf {
    dir.join(format!("{MANIFEST_FILENAME}.{}.tmp", std::process::id()))
}

impl SessionManifest {
    pub fn new(
        session_id: String,
        session_label: String,
        started_at_utc: DateTime<Utc>,
        poll_interval_ms_requested: u64,
        host: HostInfo,
    ) -> Self {
        let workload = Workload::new(&session_label);
        Self {
            schema_version: SCHEMA_VERSION,
            session_id,
            session_label,
            started_at_utc,
            ended_at_utc: None,
            poll_interval_ms_requested,
            collector_version: crate::build_info::collector_version().to_owned(),
            git_commit: crate::build_info::git_sha(),
            restart_count: 0,
            host,
            workload,
            parquet_write_failures: 0,
            timing: None,
        }
    }

    /// Read an existing manifest. An absent file is normal; malformed or
    /// unsupported manifests must stop the collector rather than being silently
    /// overwritten as a new session.
    pub fn load(dir: &Path) -> Result<Option<Self>> {
        let path = manifest_path(dir);
        let contents = match std::fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "failed to read {}",
                        crate::privacy::redact_personal_path(&path.display().to_string())
                    )
                });
            }
        };
        let manifest: Self = serde_json::from_str(&contents).with_context(|| {
            format!(
                "failed to parse {}",
                crate::privacy::redact_personal_path(&path.display().to_string())
            )
        })?;
        anyhow::ensure!(
            manifest.schema_version == SCHEMA_VERSION,
            "unsupported session manifest schema_version {}; expected {}",
            manifest.schema_version,
            SCHEMA_VERSION
        );
        Ok(Some(manifest))
    }

    /// Build the manifest for a run, adopting any prior manifest in the same
    /// directory.
    ///
    /// Restarting a collector into an existing session directory continues that
    /// session rather than starting a new one: its identity, workload, label,
    /// and cumulative persistence failures are preserved, `ended_at_utc` is
    /// cleared because the session is live again, and `restart_count`
    /// increments. Run-specific values (version, git SHA, host, poll interval)
    /// are refreshed from the current process.
    pub fn load_or_new(
        dir: &Path,
        session_id: String,
        session_label: String,
        started_at_utc: DateTime<Utc>,
        poll_interval_ms_requested: u64,
        host: HostInfo,
    ) -> Result<Self> {
        let mut manifest = Self::new(
            session_id,
            session_label,
            started_at_utc,
            poll_interval_ms_requested,
            host,
        );
        if let Some(previous) = Self::load(dir)? {
            manifest.session_id = previous.session_id;
            manifest.started_at_utc = previous.started_at_utc;
            manifest.restart_count = previous.restart_count.saturating_add(1);
            manifest.session_label = previous.session_label;
            manifest.workload = previous.workload;
            manifest.parquet_write_failures = previous.parquet_write_failures;
        }
        Ok(manifest)
    }

    /// Stamp when collection stopped and attach the timing/persistence summary.
    pub fn finalize(
        &mut self,
        ended_at_utc: DateTime<Utc>,
        timing: TimingSummary,
        parquet_write_failures: u32,
    ) {
        self.ended_at_utc = Some(ended_at_utc);
        self.timing = Some(timing);
        self.parquet_write_failures = self
            .parquet_write_failures
            .saturating_add(parquet_write_failures);
    }

    /// Serialize to `session_manifest.json` via a temp file and rename.
    ///
    /// The rename is atomic, so a kill mid-write can leave the previous manifest
    /// or the new one — never a truncated file that fails to parse.
    pub fn write_atomic(&self, dir: &Path) -> Result<()> {
        let tmp = temp_path(dir);
        let json = serde_json::to_string_pretty(self).context("failed to serialize manifest")?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .with_context(|| {
                format!(
                    "failed to create {}",
                    crate::privacy::redact_personal_path(&tmp.display().to_string())
                )
            })?;
        if let Err(error) = file
            .write_all(json.as_bytes())
            .and_then(|_| file.sync_all())
        {
            drop(file);
            let _ = std::fs::remove_file(&tmp);
            return Err(error).with_context(|| {
                format!(
                    "failed to write {}",
                    crate::privacy::redact_personal_path(&tmp.display().to_string())
                )
            });
        }
        drop(file);
        std::fs::rename(&tmp, manifest_path(dir)).with_context(|| {
            format!(
                "failed to finalize {}",
                crate::privacy::redact_personal_path(&manifest_path(dir).display().to_string())
            )
        })?;
        File::open(dir)
            .and_then(|directory| directory.sync_all())
            .with_context(|| {
                format!(
                    "failed to persist manifest rename in {}",
                    crate::privacy::redact_personal_path(&dir.display().to_string())
                )
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timing::TimingStats;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-fixtures")
            .join(format!(
                "manifest_{tag}_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn fixture(session_id: &str) -> SessionManifest {
        SessionManifest::new(
            session_id.to_owned(),
            "kcd2".to_owned(),
            Utc::now(),
            5,
            HostInfo::new(Some("RTX 5080".to_owned()), Some("580.00".to_owned())),
        )
    }

    #[test]
    fn parse_cpu_model_extracts_the_model_name_line() {
        let cpuinfo = "processor\t: 0\nvendor_id\t: AuthenticAMD\nmodel name\t: AMD Ryzen 9 7950X 16-Core Processor\ncpu MHz\t: 4500.000\n";
        assert_eq!(
            parse_cpu_model(cpuinfo).as_deref(),
            Some("AMD Ryzen 9 7950X 16-Core Processor")
        );
    }

    #[test]
    fn parse_cpu_model_handles_missing_or_empty_values() {
        assert_eq!(parse_cpu_model(""), None);
        assert_eq!(parse_cpu_model("processor\t: 0\nflags\t: fpu vme\n"), None);
        assert_eq!(parse_cpu_model("model name\t:   \n"), None);
        // A value containing a colon must not be truncated at the second one.
        assert_eq!(
            parse_cpu_model("model name\t: Weird: CPU v2\n").as_deref(),
            Some("Weird: CPU v2")
        );
    }

    #[test]
    fn host_info_falls_back_to_unknown() {
        let host = HostInfo::new(None, None);
        assert_eq!(host.gpu_name, "unknown");
        assert_eq!(host.driver, "unknown");
        assert!(!host.cpu_model.is_empty());
    }

    #[test]
    fn manifest_round_trips_through_json() {
        let manifest = fixture("kcd2_20260816");
        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: SessionManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, manifest);
        assert_eq!(parsed.schema_version, 1);
        assert_eq!(parsed.ended_at_utc, None);
        assert_eq!(parsed.timing, None);
        assert_eq!(parsed.restart_count, 0);
        assert_eq!(parsed.workload.class, "gaming");
        assert_eq!(parsed.workload.label, "kcd2");
    }

    #[test]
    fn write_atomic_leaves_parseable_json_and_no_temp_file() {
        let dir = temp_dir("atomic");
        fixture("s1").write_atomic(&dir).unwrap();

        let path = dir.join(MANIFEST_FILENAME);
        assert!(path.is_file());
        assert!(
            !temp_path(&dir).exists(),
            "temp file must not survive a successful write"
        );
        let reloaded = SessionManifest::load(&dir)
            .expect("manifest should load")
            .expect("manifest should exist");
        assert_eq!(reloaded.session_id, "s1");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn finalize_sets_end_time_and_timing() {
        let dir = temp_dir("finalize");
        let mut manifest = fixture("s2");
        let mut stats = TimingStats::new(5);
        stats.record(std::time::Instant::now());

        manifest.finalize(Utc::now(), stats.summary(), 0);
        manifest.write_atomic(&dir).unwrap();

        let reloaded = SessionManifest::load(&dir).unwrap().unwrap();
        assert!(reloaded.ended_at_utc.is_some());
        let timing = reloaded.timing.expect("timing summary should be attached");
        assert_eq!(timing.poll_interval_ms_requested, 5);
        assert_eq!(timing.sample_count, 1);
        assert_eq!(reloaded.parquet_write_failures, 0);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn restart_preserves_identity_and_bumps_count() {
        let dir = temp_dir("restart");
        let mut first = fixture("original_id");
        let original_start = first.started_at_utc;
        first.parquet_write_failures = 2;
        first.finalize(Utc::now(), TimingStats::new(5).summary(), 1);
        first.write_atomic(&dir).unwrap();

        // A later process resolves a different id/start, but the directory already
        // has a session in it.
        let second = SessionManifest::load_or_new(
            &dir,
            "some_other_id".to_owned(),
            "different_label".to_owned(),
            Utc::now(),
            5,
            HostInfo::new(None, None),
        )
        .unwrap();

        assert_eq!(
            second.session_id, "original_id",
            "identity must be preserved"
        );
        assert_eq!(
            second.started_at_utc, original_start,
            "session start must be preserved across restarts"
        );
        assert_eq!(second.restart_count, 1);
        assert_eq!(second.session_label, "kcd2");
        assert_eq!(second.workload.label, "kcd2");
        assert_eq!(second.parquet_write_failures, 3);
        assert_eq!(
            second.ended_at_utc, None,
            "a restarted session is live again"
        );

        second.write_atomic(&dir).unwrap();
        let third = SessionManifest::load_or_new(
            &dir,
            "ignored".to_owned(),
            "kcd2".to_owned(),
            Utc::now(),
            5,
            HostInfo::new(None, None),
        )
        .unwrap();
        assert_eq!(third.restart_count, 2);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_or_new_starts_fresh_when_directory_is_empty_but_rejects_corruption() {
        let dir = temp_dir("fresh");
        let fresh = SessionManifest::load_or_new(
            &dir,
            "new_id".to_owned(),
            "re2r".to_owned(),
            Utc::now(),
            5,
            HostInfo::new(None, None),
        )
        .unwrap();
        assert_eq!(fresh.session_id, "new_id");
        assert_eq!(fresh.restart_count, 0);

        // A truncated manifest must not be replaced silently: the existing data
        // could be a real, interrupted session that needs operator recovery.
        std::fs::write(dir.join(MANIFEST_FILENAME), b"{\"schema_vers").unwrap();
        assert!(SessionManifest::load_or_new(
            &dir,
            "new_id2".to_owned(),
            "re2r".to_owned(),
            Utc::now(),
            5,
            HostInfo::new(None, None),
        )
        .is_err());

        std::fs::write(
            dir.join(MANIFEST_FILENAME),
            serde_json::json!({ "schema_version": SCHEMA_VERSION + 1 }).to_string(),
        )
        .unwrap();
        assert!(SessionManifest::load(&dir).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
