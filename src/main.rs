// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::Result;
use chrono::{DateTime, Utc};
use gaming_telemetry::build_info::git_sha;
use gaming_telemetry::cpu::CpuMonitor;
use gaming_telemetry::manifest::{HostInfo, SessionManifest};
use gaming_telemetry::privacy;
use gaming_telemetry::session;
use gaming_telemetry::timing::TimingStats;
use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};
use nvml_wrapper::Nvml;
use polars::prelude::*;
use sentry::ClientInitGuard;
use std::borrow::Cow;
use std::fs::{remove_file, rename, File};
use std::path::{Path, PathBuf};
use tokio::task::JoinSet;
use tokio::time::{interval, Duration, MissedTickBehavior};

fn resolve_sentry_release(git_sha: &str) -> String {
    if let Some(release) = std::env::var("SENTRY_RELEASE")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        return release;
    }

    if !git_sha.trim().is_empty() && git_sha != "unknown" {
        return format!("gaming-telemetry@{git_sha}");
    }

    if let Some(release) = sentry::release_name!() {
        let release = release.into_owned();
        if !release.trim().is_empty() {
            return release;
        }
    }

    // Embed the release SHA (when available via the passed git_sha or prior checks)
    // before any final fallback. Avoid "@unknown" to prevent mismatch with the
    // Sentry release workflow which always creates names with the actual SHA
    // (e.g. gaming-telemetry@<sha> from git rev-parse in CI).
    "gaming-telemetry".to_owned()
}

fn init_sentry() -> Option<ClientInitGuard> {
    let dsn = std::env::var("SENTRY_AUTH_TOKEN")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())?;
    let git_sha = git_sha();
    let release = resolve_sentry_release(&git_sha);
    let environment = std::env::var("SENTRY_ENVIRONMENT")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "local".to_owned());
    let parsed_dsn = match dsn.parse() {
        Ok(dsn) => dsn,
        Err(error) => {
            eprintln!("Sentry disabled: invalid SENTRY_AUTH_TOKEN ({error})");
            return None;
        }
    };

    let guard = sentry::init(sentry::ClientOptions {
        dsn: Some(parsed_dsn),
        release: Some(Cow::Owned(release)),
        environment: Some(Cow::Owned(environment)),
        sample_rate: 1.0,
        traces_sample_rate: 0.0,
        default_integrations: true,
        ..Default::default()
    });

    Some(guard)
}

#[derive(Debug, Clone)]
struct GpuSample {
    timestamp: DateTime<Utc>,
    session_label: String,
    power_usage_mw: u32,
    temperature_c: u32,
    graphics_clock_mhz: u32,
    memory_clock_mhz: u32,
    pcie_rx_throughput_kbps: u32,
    pcie_tx_throughput_kbps: u32,
    pstate: u32,
    throttle_reasons: u64,
    fan_speed_perc: u32,
    memory_used_mb: u64,
    memory_total_mb: u64,
    encoder_util_perc: u32,
    decoder_util_perc: u32,
    // CPU telemetry
    cpu_tctl_c: f32,
    cpu_ccd1_c: f32,
    cpu_ccd2_c: f32,
    cpu_package_power_w: f32,
}

const BUFFER_SIZE: usize = 2000; // ~10 seconds of data at default 5ms intervals
/// Cap outstanding async Parquet writes so a slow disk cannot queue unbounded batches.
const MAX_IN_FLIGHT_WRITES: usize = 2;

fn next_batch_id(batch_id: u32) -> Result<u32> {
    batch_id
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("batch ID namespace exhausted; start a new SESSION_DIR"))
}

fn write_to_parquet(samples: Vec<GpuSample>, batch_id: u32, output_dir: &Path) -> Result<()> {
    let timestamps: Vec<i64> = samples
        .iter()
        .map(|s| s.timestamp.timestamp_millis())
        .collect();
    let session_labels: Vec<String> = samples.iter().map(|s| s.session_label.clone()).collect();
    let power: Vec<u32> = samples.iter().map(|s| s.power_usage_mw).collect();
    let temp: Vec<u32> = samples.iter().map(|s| s.temperature_c).collect();
    let graphics_clock: Vec<u32> = samples.iter().map(|s| s.graphics_clock_mhz).collect();
    let memory_clock: Vec<u32> = samples.iter().map(|s| s.memory_clock_mhz).collect();
    let pcie_rx: Vec<u32> = samples.iter().map(|s| s.pcie_rx_throughput_kbps).collect();
    let pcie_tx: Vec<u32> = samples.iter().map(|s| s.pcie_tx_throughput_kbps).collect();
    let pstate: Vec<u32> = samples.iter().map(|s| s.pstate).collect();
    let throttle: Vec<u64> = samples.iter().map(|s| s.throttle_reasons).collect();
    let fan: Vec<u32> = samples.iter().map(|s| s.fan_speed_perc).collect();
    let mem_used: Vec<u64> = samples.iter().map(|s| s.memory_used_mb).collect();
    let mem_total: Vec<u64> = samples.iter().map(|s| s.memory_total_mb).collect();
    let enc_util: Vec<u32> = samples.iter().map(|s| s.encoder_util_perc).collect();
    let dec_util: Vec<u32> = samples.iter().map(|s| s.decoder_util_perc).collect();
    let cpu_tctl: Vec<f32> = samples.iter().map(|s| s.cpu_tctl_c).collect();
    let cpu_ccd1: Vec<f32> = samples.iter().map(|s| s.cpu_ccd1_c).collect();
    let cpu_ccd2: Vec<f32> = samples.iter().map(|s| s.cpu_ccd2_c).collect();
    let cpu_power: Vec<f32> = samples.iter().map(|s| s.cpu_package_power_w).collect();

    let mut df = df!(
        "timestamp_ms" => timestamps,
        "session_label" => session_labels,
        "power_usage_mw" => power,
        "temperature_c" => temp,
        "graphics_clock_mhz" => graphics_clock,
        "memory_clock_mhz" => memory_clock,
        "pcie_rx_kbps" => pcie_rx,
        "pcie_tx_kbps" => pcie_tx,
        "pstate" => pstate,
        "throttle_reasons_bitmask" => throttle,
        "fan_speed_perc" => fan,
        "memory_used_mb" => mem_used,
        "memory_total_mb" => mem_total,
        "encoder_util_perc" => enc_util,
        "decoder_util_perc" => dec_util,
        "cpu_tctl_c" => cpu_tctl,
        "cpu_ccd1_c" => cpu_ccd1,
        "cpu_ccd2_c" => cpu_ccd2,
        "cpu_package_power_w" => cpu_power,
    )?;

    let filename = output_dir.join(format!(
        "{}{}{}",
        session::BATCH_PREFIX,
        batch_id,
        session::BATCH_SUFFIX
    ));
    // A failed writer must never publish an incomplete file under the final batch
    // name: restart scanning treats final names as complete batches. The temporary
    // name cannot match `highest_batch_id` and is published only after a complete,
    // flushed write.
    let temporary = output_dir.join(format!(
        ".{}{}{}.{}.tmp",
        session::BATCH_PREFIX,
        batch_id,
        session::BATCH_SUFFIX,
        std::process::id()
    ));
    let mut file = File::create(&temporary)?;
    if let Err(error) = (|| -> Result<()> {
        ParquetWriter::new(&mut file).finish(&mut df)?;
        file.sync_all()?;
        Ok(())
    })() {
        drop(file);
        let _ = remove_file(&temporary);
        return Err(error);
    }
    drop(file);
    if let Err(error) = rename(&temporary, &filename) {
        let _ = remove_file(&temporary);
        return Err(error.into());
    }

    println!(
        "Wrote batch {} to {}",
        batch_id,
        privacy::redact_personal_path(&filename.display().to_string())
    );
    Ok(())
}

fn spawn_parquet_write(
    in_flight: &mut JoinSet<Result<()>>,
    samples: Vec<GpuSample>,
    batch_id: u32,
    output_dir: PathBuf,
) {
    let hub = sentry::Hub::current();
    in_flight.spawn(async move {
        match tokio::task::spawn_blocking(move || {
            sentry::Hub::run(hub, || write_to_parquet(samples, batch_id, &output_dir))
        })
        .await
        {
            Ok(result) => result,
            Err(join_err) => Err(anyhow::anyhow!("parquet write task panicked: {}", join_err)),
        }
    });
}

fn record_write_result(res: Result<Result<()>, tokio::task::JoinError>, write_failures: &mut u32) {
    match res {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            *write_failures += 1;
            let redacted = privacy::redact_personal_path(&format!("{:?}", e));
            eprintln!("Failed to write to Parquet: {}", redacted);
            sentry::capture_message(&redacted, sentry::Level::Error);
        }
        Err(e) => {
            *write_failures += 1;
            eprintln!("In-flight parquet write task failed: {}", e);
        }
    }
}

/// Reap finished writes; if still at capacity, await the next one to finish (backpressure).
/// Returns `true` if Ctrl+C arrived while waiting so the outer loop can shut down.
async fn reclaim_in_flight(in_flight: &mut JoinSet<Result<()>>, write_failures: &mut u32) -> bool {
    while let Some(res) = in_flight.try_join_next() {
        record_write_result(res, write_failures);
    }
    if in_flight.len() < MAX_IN_FLIGHT_WRITES {
        return false;
    }
    tokio::select! {
        res = in_flight.join_next() => {
            if let Some(res) = res {
                record_write_result(res, write_failures);
            }
            false
        }
        _ = tokio::signal::ctrl_c() => true,
    }
}

async fn drain_in_flight(in_flight: &mut JoinSet<Result<()>>, write_failures: &mut u32) {
    while let Some(res) = in_flight.join_next().await {
        record_write_result(res, write_failures);
    }
}

/// Flush the tail buffer, drain outstanding writes, then finalize the manifest.
///
/// Both shutdown paths (Ctrl+C, and Ctrl+C during write backpressure) converge
/// here, so the manifest is finalized and rewritten exactly once per run.
async fn perform_shutdown(
    buffer: Vec<GpuSample>,
    batch_counter: u32,
    output_dir: &Path,
    in_flight: &mut JoinSet<Result<()>>,
    write_failures: &mut u32,
    manifest: &mut SessionManifest,
    timing: &TimingStats,
) -> Result<()> {
    // This is the last instant the collector may have produced telemetry. Storage
    // drain can take much longer and must not inflate the capture's end time.
    let capture_ended_at_utc = Utc::now();
    if !buffer.is_empty() {
        let new_batch_id = next_batch_id(batch_counter)?;
        if let Err(e) = write_to_parquet(buffer, new_batch_id, output_dir) {
            *write_failures += 1;
            let redacted = privacy::redact_personal_path(&format!("{:?}", e));
            eprintln!("Failed to write final batch: {}", redacted);
            sentry::capture_message(&redacted, sentry::Level::Error);
        }
    }
    drain_in_flight(in_flight, write_failures).await;

    // Finalize before any failure bail-out: a run that lost batches still deserves
    // an accurate manifest describing what it acquired and what failed to persist.
    manifest.finalize(capture_ended_at_utc, timing.summary(), *write_failures);
    if let Err(e) = manifest.write_atomic(output_dir) {
        let redacted = privacy::redact_personal_path(&format!("{:?}", e));
        eprintln!("Failed to finalize session manifest: {}", redacted);
        sentry::capture_message(&redacted, sentry::Level::Error);
        // A missing on-disk finalize (`ended_at_utc: null`) is an unclean exit;
        // do not report graceful success or a zero exit status.
        return Err(e);
    }

    if *write_failures > 0 {
        anyhow::bail!("Graceful shutdown finished with {write_failures} parquet write failure(s)");
    }
    println!("Graceful shutdown complete.");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let _sentry_guard = init_sentry();

    let mut session_label = session::resolve_label();

    let nvml = Nvml::init()?;
    let device = nvml.device_by_index(0)?; // Target first GPU

    // Configurable poll interval via environment variable
    let poll_interval_ms =
        session::parse_poll_interval_ms(std::env::var("POLL_INTERVAL_MS").ok().as_deref())?;

    let output_dir = session::resolve_dir()?;
    let _session_lock = session::acquire_exclusive(&output_dir)?;
    let started_at = Utc::now();
    let session_id = session::session_id(
        &session_label,
        &started_at.format("%Y%m%dT%H%M%S%.fZ").to_string(),
    );

    // Do not attach a new manifest to legacy batches that have no manifest to
    // describe their provenance. Operators must migrate or separate that data.
    let mut batch_counter = session::highest_batch_id(&output_dir)?;
    anyhow::ensure!(
        batch_counter == 0 || SessionManifest::load(&output_dir)?.is_some(),
        "SESSION_DIR contains existing telemetry batches but no valid session manifest; use a new directory or restore the manifest"
    );
    anyhow::ensure!(
        batch_counter < u32::MAX,
        "batch ID namespace exhausted; start a new SESSION_DIR"
    );

    let host = HostInfo::new(device.name().ok(), nvml.sys_driver_version().ok());
    let mut manifest = SessionManifest::load_or_new(
        &output_dir,
        session_id,
        session_label.clone(),
        started_at,
        poll_interval_ms,
        host,
    )?;
    // A restarted directory remains one labeled workload/session. Preserve the
    // established identity instead of writing new batches with a conflicting tag.
    session_label = manifest.session_label.clone();
    let mut buffer = Vec::with_capacity(BUFFER_SIZE);
    let mut interval = interval(Duration::from_millis(poll_interval_ms));
    // After write backpressure, do not burst-catch every missed 5ms tick.
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    // Publish only after the fallible resume scan succeeds, so an unreadable
    // existing directory cannot overwrite its prior completed manifest.
    manifest.write_atomic(&output_dir)?;
    let mut cpu_monitor = CpuMonitor::new();
    let mut timing = TimingStats::new(poll_interval_ms);
    let mut in_flight: JoinSet<Result<()>> = JoinSet::new();
    let mut write_failures: u32 = 0;

    println!(
        "Starting GPU telemetry: poll_interval_ms={} session_label={:?} session_id={:?}",
        poll_interval_ms, session_label, manifest.session_id
    );
    println!(
        "Session directory: {}",
        privacy::redact_personal_path(&output_dir.display().to_string())
    );
    if manifest.restart_count > 0 {
        println!(
            "Resuming session (restart #{}) from batch {}",
            manifest.restart_count, batch_counter
        );
    }
    println!("Press Ctrl+C to stop gracefully.");

    loop {
        tokio::select! {
            tick = interval.tick() => {
                timing.record_scheduled_tick(tick.into_std());

                let power_usage = device.power_usage().unwrap_or(0);
                let temperature = device.temperature(TemperatureSensor::Gpu).unwrap_or(0);
                let graphics_clock = device.clock_info(Clock::Graphics).unwrap_or(0);
                let memory_clock = device.clock_info(Clock::Memory).unwrap_or(0);

                let pcie_rx = device.pcie_throughput(nvml_wrapper::enum_wrappers::device::PcieUtilCounter::Receive).unwrap_or(0);
                let pcie_tx = device.pcie_throughput(nvml_wrapper::enum_wrappers::device::PcieUtilCounter::Send).unwrap_or(0);
                let pstate = device.performance_state().map(|p| p as u32).unwrap_or(0);
                let throttle = device.current_throttle_reasons().map(|t| t.bits()).unwrap_or(0);
                let fan = device.fan_speed(0).unwrap_or(0);
                let mem_info = device.memory_info();

                // Encoder/Decoder utilization
                let encoder_util = device.encoder_utilization().map(|u| u.utilization).unwrap_or(0);
                let decoder_util = device.decoder_utilization().map(|u| u.utilization).unwrap_or(0);

                // CPU telemetry (poll for time-delta power calculation)
                let (cpu_tctl_c, cpu_package_power_w) = cpu_monitor.poll();
                let cpu_ccd1_c = cpu_monitor.read_ccd1();
                let cpu_ccd2_c = cpu_monitor.read_ccd2();

                let sample = GpuSample {
                    timestamp: Utc::now(),
                    session_label: session_label.clone(),
                    power_usage_mw: power_usage,
                    temperature_c: temperature,
                    graphics_clock_mhz: graphics_clock,
                    memory_clock_mhz: memory_clock,
                    pcie_rx_throughput_kbps: pcie_rx,
                    pcie_tx_throughput_kbps: pcie_tx,
                    pstate,
                    throttle_reasons: throttle,
                    fan_speed_perc: fan,
                    memory_used_mb: mem_info.as_ref().map(|m| m.used / 1024 / 1024).unwrap_or(0),
                    memory_total_mb: mem_info.as_ref().map(|m| m.total / 1024 / 1024).unwrap_or(0),
                    encoder_util_perc: encoder_util,
                    decoder_util_perc: decoder_util,
                    cpu_tctl_c,
                    cpu_ccd1_c,
                    cpu_ccd2_c,
                    cpu_package_power_w,
                };

                // Measure when telemetry was actually obtained, not the Tokio
                // scheduler's nominal deadline. This includes collection stalls.
                timing.record(std::time::Instant::now());

                buffer.push(sample);

                if buffer.len() >= BUFFER_SIZE {
                    if reclaim_in_flight(&mut in_flight, &mut write_failures).await {
                        println!("\nShutdown signal received during write backpressure...");
                        perform_shutdown(
                            std::mem::take(&mut buffer),
                            batch_counter,
                            &output_dir,
                            &mut in_flight,
                            &mut write_failures,
                            &mut manifest,
                            &timing,
                        ).await?;
                        break;
                    }
                    let samples_to_write =
                        std::mem::replace(&mut buffer, Vec::with_capacity(BUFFER_SIZE));
                    batch_counter = next_batch_id(batch_counter)?;
                    spawn_parquet_write(
                        &mut in_flight,
                        samples_to_write,
                        batch_counter,
                        output_dir.clone(),
                    );
                }
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\nShutdown signal received. Finalizing last batch...");
                perform_shutdown(
                    buffer,
                    batch_counter,
                    &output_dir,
                    &mut in_flight,
                    &mut write_failures,
                    &mut manifest,
                    &timing,
                ).await?;
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_fixture(label: &str) -> GpuSample {
        GpuSample {
            timestamp: Utc::now(),
            session_label: label.to_owned(),
            power_usage_mw: 120_000,
            temperature_c: 65,
            graphics_clock_mhz: 2500,
            memory_clock_mhz: 10000,
            pcie_rx_throughput_kbps: 100,
            pcie_tx_throughput_kbps: 50,
            pstate: 0,
            throttle_reasons: 0,
            fan_speed_perc: 40,
            memory_used_mb: 8_000,
            memory_total_mb: 16_000,
            encoder_util_perc: 0,
            decoder_util_perc: 0,
            cpu_tctl_c: 55.0,
            cpu_ccd1_c: 50.0,
            cpu_ccd2_c: 51.0,
            cpu_package_power_w: 80.0,
        }
    }

    #[test]
    fn write_to_parquet_emits_labeled_batch_file() {
        let base = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-fixtures");
        let tmp = base.join(format!(
            "gt_parquet_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        let batch_id = 99_001;
        let path = tmp.join(format!(
            "{}{}{}",
            session::BATCH_PREFIX,
            batch_id,
            session::BATCH_SUFFIX
        ));
        let _ = std::fs::remove_file(&path);
        write_to_parquet(
            vec![sample_fixture("kcd2"), sample_fixture("kcd2")],
            batch_id,
            &tmp,
        )
        .expect("parquet write");
        assert!(path.is_file());
        assert!(
            std::fs::read_dir(&tmp)
                .unwrap()
                .flatten()
                .all(|entry| !entry.file_name().to_string_lossy().ends_with(".tmp")),
            "a successful write publishes only the final batch name"
        );

        // Verify the session_label column is written for every row.
        let df = LazyFrame::scan_parquet(&path, ScanArgsParquet::default())
            .unwrap()
            .select(&[col("session_label")])
            .collect()
            .unwrap();
        let labels = df.column("session_label").unwrap().str().unwrap();
        assert_eq!(labels.len(), 2);
        assert!(labels.into_iter().all(|opt| opt == Some("kcd2")));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    /// The batch filename the collector writes must be the one `highest_batch_id`
    /// parses, or restart numbering silently breaks.
    #[test]
    fn written_batch_filename_is_discoverable_by_session_scan() {
        let tmp = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-fixtures")
            .join(format!(
                "gt_batch_scan_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&tmp).unwrap();

        write_to_parquet(vec![sample_fixture("kcd2")], 4, &tmp).expect("parquet write");
        assert_eq!(session::highest_batch_id(&tmp).unwrap(), 4);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn record_write_result_counts_io_failures() {
        let mut fails = 0u32;
        record_write_result(Ok(Ok(())), &mut fails);
        assert_eq!(fails, 0);
        record_write_result(Ok(Err(anyhow::anyhow!("disk full"))), &mut fails);
        assert_eq!(fails, 1);
        // JoinError is hard to construct without panicking a task; skip Err arm here.
    }

    #[test]
    fn test_sentry_helpers_env_resolution_and_init() {
        // All env-mutating coverage for git_sha/resolve/init in one test to avoid
        // parallel test races on process env (std::env::{set,remove}_var are unsafe
        // and racy). This single test exercises the branches (including the "good sha"
        // fast-path) that give codecov for the Sentry integration (#17).
        unsafe {
            std::env::set_var("AGENTOS_GIT_SHA", "abc123def");
        }
        assert_eq!(git_sha(), "abc123def");
        unsafe {
            std::env::remove_var("AGENTOS_GIT_SHA");
        }

        unsafe {
            std::env::set_var("SENTRY_RELEASE", "gaming-telemetry@myrel");
        }
        assert_eq!(resolve_sentry_release("ignored"), "gaming-telemetry@myrel");
        unsafe {
            std::env::remove_var("SENTRY_RELEASE");
        }

        // Cover the explicit-sha fast path (no SENTRY_RELEASE) sequentially.
        unsafe {
            std::env::remove_var("SENTRY_RELEASE");
        }
        assert_eq!(
            resolve_sentry_release("feedface"),
            "gaming-telemetry@feedface"
        );

        unsafe {
            std::env::remove_var("SENTRY_RELEASE");
        }
        let r = resolve_sentry_release("unknown");
        assert!(r.starts_with("gaming-telemetry"));

        unsafe {
            std::env::remove_var("SENTRY_AUTH_TOKEN");
        }
        let guard = init_sentry();
        assert!(guard.is_none());

        unsafe {
            std::env::set_var("SENTRY_AUTH_TOKEN", "https://key@00000.ingest.sentry.io/0");
            std::env::set_var("SENTRY_ENVIRONMENT", "ci-test");
        }
        let guard = init_sentry();
        assert!(guard.is_some());
        unsafe {
            std::env::remove_var("SENTRY_AUTH_TOKEN");
            std::env::remove_var("SENTRY_ENVIRONMENT");
        }
    }
}
