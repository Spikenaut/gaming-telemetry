use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::verify::VerifyError;
use crate::verify::types::RuntimeThresholds;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Json,
    Text,
}

#[derive(Debug, Clone, Parser)]
#[command(name = "verify_cyberpunk")]
#[command(about = "Read-only verifier for a Cyberpunk 2077 telemetry workload")]
pub struct Args {
    #[arg(long)]
    pub telemetry: Option<PathBuf>,
    #[arg(long)]
    pub out: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    pub format: OutputFormat,
    #[arg(long, default_value = "", value_parser = parse_runtime_thresholds)]
    pub runtime_thresholds: RuntimeThresholds,
    #[arg(long)]
    pub debug: bool,
    #[arg(long)]
    pub dry_run: bool,
}

impl Args {
    pub fn validate(&self) -> Result<(), VerifyError> {
        if self.dry_run && self.out.is_some() {
            return Err(VerifyError::Usage(
                "--dry-run refuses file output; use stdout by omitting --out".to_string(),
            ));
        }
        Ok(())
    }
}

fn parse_runtime_thresholds(input: &str) -> Result<RuntimeThresholds, String> {
    if input.trim().is_empty() {
        return Ok(RuntimeThresholds::default());
    }

    let value: serde_json::Value =
        serde_json::from_str(input).map_err(|err| format!("invalid thresholds JSON: {err}"))?;

    let mut thresholds = RuntimeThresholds::default();
    if let Some(v) = value.get("avg_gpu_w").and_then(|v| v.as_f64()) {
        thresholds.avg_gpu_w = v;
    }
    if let Some(v) = value.get("max_gpu_w").and_then(|v| v.as_f64()) {
        thresholds.max_gpu_w = v;
    }
    if let Some(v) = value.get("max_vram_mb").and_then(|v| v.as_f64()) {
        thresholds.max_vram_mb = v;
    }
    if let Some(v) = value.get("max_pcie_rx_mb_s").and_then(|v| v.as_f64()) {
        thresholds.max_pcie_rx_mb_s = v;
    }
    if let Some(v) = value.get("avg_cpu_w").and_then(|v| v.as_f64()) {
        thresholds.avg_cpu_w = v;
    }
    Ok(thresholds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_thresholds() {
        let thresholds = parse_runtime_thresholds("").unwrap();
        assert_eq!(thresholds, RuntimeThresholds::default());
    }

    #[test]
    fn parses_custom_thresholds() {
        let thresholds = parse_runtime_thresholds(
            r#"{"avg_gpu_w":210,"max_gpu_w":290,"max_vram_mb":12000,"max_pcie_rx_mb_s":900,"avg_cpu_w":100}"#,
        )
        .unwrap();
        assert_eq!(thresholds.avg_gpu_w, 210.0);
        assert_eq!(thresholds.max_gpu_w, 290.0);
        assert_eq!(thresholds.max_vram_mb, 12000.0);
        assert_eq!(thresholds.max_pcie_rx_mb_s, 900.0);
        assert_eq!(thresholds.avg_cpu_w, 100.0);
    }
}
