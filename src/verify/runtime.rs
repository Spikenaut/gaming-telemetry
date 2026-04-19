use std::path::Path;

use polars::prelude::*;

use crate::verify::types::{ClaimVerdict, RuntimeMetrics, RuntimeSection, RuntimeThresholds};

#[derive(Debug)]
pub enum RuntimeAuditError {
    RuntimeParse(String),
    Input(String),
}

pub fn runtime_audit(
    telemetry: Option<&Path>,
    thresholds: &RuntimeThresholds,
    debug: bool,
) -> Result<Option<RuntimeSection>, RuntimeAuditError> {
    let Some(path) = telemetry else {
        return Ok(None);
    };

    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let metrics = match extension.as_str() {
        "csv" => parse_csv_metrics(path),
        "parquet" => parse_parquet_metrics(path),
        _ => Err(RuntimeAuditError::Input(format!(
            "unsupported telemetry format for {}",
            path.display()
        ))),
    };

    match metrics {
        Ok(metrics) => {
            let checks = [
                metrics.avg_gpu_w >= thresholds.avg_gpu_w
                    || metrics.max_gpu_w >= thresholds.max_gpu_w,
                metrics.max_vram_mb >= thresholds.max_vram_mb,
                metrics.max_pcie_rx_mb_s >= thresholds.max_pcie_rx_mb_s,
            ];
            let pass_count = checks.into_iter().filter(|ok| *ok).count();
            let verdict = if pass_count >= 2 {
                ClaimVerdict::Pass
            } else {
                ClaimVerdict::Inconclusive
            };

            if debug {
                eprintln!(
                    "[verify_cyberpunk][debug] runtime metrics avg_gpu_w={} max_gpu_w={} max_vram_mb={} max_pcie_rx_mb_s={} verdict={:?}",
                    metrics.avg_gpu_w,
                    metrics.max_gpu_w,
                    metrics.max_vram_mb,
                    metrics.max_pcie_rx_mb_s,
                    verdict
                );
            }

            Ok(Some(RuntimeSection {
                telemetry_path: Some(path.display().to_string()),
                file_type: Some(extension),
                thresholds: thresholds.clone(),
                metrics: Some(metrics),
                runtime_parse_error: None,
                runtime_corroboration: Some(verdict),
            }))
        }
        Err(RuntimeAuditError::RuntimeParse(message)) => Ok(Some(RuntimeSection {
            telemetry_path: Some(path.display().to_string()),
            file_type: Some(extension),
            thresholds: thresholds.clone(),
            metrics: None,
            runtime_parse_error: Some(message),
            runtime_corroboration: Some(ClaimVerdict::Inconclusive),
        })),
        Err(err) => Err(err),
    }
}

fn parse_csv_metrics(path: &Path) -> Result<RuntimeMetrics, RuntimeAuditError> {
    let mut reader = csv::Reader::from_path(path).map_err(|err| {
        RuntimeAuditError::RuntimeParse(format!("failed to open CSV {}: {err}", path.display()))
    })?;

    let headers = reader
        .headers()
        .map_err(|err| {
            RuntimeAuditError::RuntimeParse(format!(
                "failed to read CSV header {}: {err}",
                path.display()
            ))
        })?
        .clone();

    let power_idx = find_header(&headers, &["gpu_power_w", "power_usage_mw"]);
    let vram_idx = find_header(&headers, &["vram_mb", "memory_used_mb"]);
    let pcie_idx = find_header(&headers, &["pcie_rx_mb_s", "pcie_rx_kbps"]);

    let mut gpu = Vec::new();
    let mut vram = Vec::new();
    let mut pcie = Vec::new();

    for row in reader.records() {
        let row = row.map_err(|err| {
            RuntimeAuditError::RuntimeParse(format!(
                "failed to read CSV row {}: {err}",
                path.display()
            ))
        })?;
        if let Some(idx) = power_idx {
            let value = row.get(idx).unwrap_or("0").parse::<f64>().unwrap_or(0.0);
            gpu.push(if headers.get(idx) == Some("power_usage_mw") {
                value / 1000.0
            } else {
                value
            });
        }
        if let Some(idx) = vram_idx {
            vram.push(row.get(idx).unwrap_or("0").parse::<f64>().unwrap_or(0.0));
        }
        if let Some(idx) = pcie_idx {
            let value = row.get(idx).unwrap_or("0").parse::<f64>().unwrap_or(0.0);
            pcie.push(if headers.get(idx) == Some("pcie_rx_kbps") {
                value / 1024.0
            } else {
                value
            });
        }
    }

    build_metrics(gpu, vram, pcie)
}

fn parse_parquet_metrics(path: &Path) -> Result<RuntimeMetrics, RuntimeAuditError> {
    let df = LazyFrame::scan_parquet(path, ScanArgsParquet::default())
        .map_err(|err| {
            RuntimeAuditError::RuntimeParse(format!(
                "failed to scan parquet {}: {err}",
                path.display()
            ))
        })?
        .collect()
        .map_err(|err| {
            RuntimeAuditError::RuntimeParse(format!(
                "failed to collect parquet {}: {err}",
                path.display()
            ))
        })?;

    let gpu = df
        .column("power_usage_mw")
        .map_err(|err| {
            RuntimeAuditError::RuntimeParse(format!(
                "missing power_usage_mw in {}: {err}",
                path.display()
            ))
        })?
        .u32()
        .map_err(|err| {
            RuntimeAuditError::RuntimeParse(format!(
                "invalid power_usage_mw column {}: {err}",
                path.display()
            ))
        })?
        .into_iter()
        .flatten()
        .map(|value| value as f64 / 1000.0)
        .collect();
    let vram = df
        .column("memory_used_mb")
        .map_err(|err| {
            RuntimeAuditError::RuntimeParse(format!(
                "missing memory_used_mb in {}: {err}",
                path.display()
            ))
        })?
        .u64()
        .map_err(|err| {
            RuntimeAuditError::RuntimeParse(format!(
                "invalid memory_used_mb column {}: {err}",
                path.display()
            ))
        })?
        .into_iter()
        .flatten()
        .map(|value| value as f64)
        .collect();
    let pcie = df
        .column("pcie_rx_kbps")
        .map_err(|err| {
            RuntimeAuditError::RuntimeParse(format!(
                "missing pcie_rx_kbps in {}: {err}",
                path.display()
            ))
        })?
        .u32()
        .map_err(|err| {
            RuntimeAuditError::RuntimeParse(format!(
                "invalid pcie_rx_kbps column {}: {err}",
                path.display()
            ))
        })?
        .into_iter()
        .flatten()
        .map(|value| value as f64 / 1024.0)
        .collect();

    build_metrics(gpu, vram, pcie)
}

fn build_metrics(
    gpu: Vec<f64>,
    vram: Vec<f64>,
    pcie: Vec<f64>,
) -> Result<RuntimeMetrics, RuntimeAuditError> {
    if gpu.is_empty() || vram.is_empty() || pcie.is_empty() {
        return Err(RuntimeAuditError::RuntimeParse(
            "telemetry data did not contain enough samples to compute corroboration metrics"
                .to_string(),
        ));
    }

    Ok(RuntimeMetrics {
        avg_gpu_w: average(&gpu),
        max_gpu_w: max(&gpu),
        avg_vram_mb: average(&vram),
        max_vram_mb: max(&vram),
        avg_pcie_rx_mb_s: average(&pcie),
        max_pcie_rx_mb_s: max(&pcie),
    })
}

fn average(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn max(values: &[f64]) -> f64 {
    values.iter().copied().fold(f64::MIN, f64::max)
}

fn find_header(headers: &csv::StringRecord, names: &[&str]) -> Option<usize> {
    names
        .iter()
        .find_map(|name| headers.iter().position(|candidate| candidate == *name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_runtime_metrics() {
        let metrics = build_metrics(
            vec![200.0, 300.0],
            vec![10000.0, 12000.0],
            vec![1000.0, 1100.0],
        )
        .unwrap();
        assert_eq!(metrics.max_gpu_w, 300.0);
        assert_eq!(metrics.max_vram_mb, 12000.0);
    }
}
