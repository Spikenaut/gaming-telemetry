pub mod acf;
pub mod cli;
pub mod config;
pub mod crowd;
pub mod discovery;
pub mod dlss;
pub mod fs;
pub mod mods;
pub mod render;
pub mod runtime;
pub mod schema;
pub mod types;
pub mod verdict;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;

use chrono::Utc;
use thiserror::Error;

use crate::verify::cli::Args;
use crate::verify::config::config_audit;
use crate::verify::crowd::crowd_profile_audit;
use crate::verify::discovery::discover_environment;
use crate::verify::dlss::dlss_audit;
use crate::verify::mods::mods_audit;
use crate::verify::path::path_tracing_audit;
use crate::verify::runtime::runtime_audit;
use crate::verify::schema::{SCHEMA_VERSION, WORKLOAD_PROFILE};
use crate::verify::types::{OverallStatus, Sidecar};
use crate::verify::verdict::finalize_verdict;

mod path;

#[derive(Debug, Error)]
pub enum VerifyError {
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Input(String),
    #[error("{0}")]
    RuntimeParse(String),
    #[error("{0}")]
    Internal(String),
}

impl VerifyError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Usage(_) | Self::Input(_) => 2,
            Self::RuntimeParse(_) => 3,
            Self::Internal(_) => 1,
        }
    }
}

pub fn run(args: Args) -> Result<Sidecar, VerifyError> {
    args.validate()?;

    let discovery = discover_environment(args.debug)?;
    let install = discovery::install_audit(&discovery, args.debug)?;
    let config = config_audit(&discovery, args.debug)?;
    let mods = mods_audit(&discovery, args.debug)?;
    let crowd_profile = crowd_profile_audit(&discovery, &config, &mods, args.debug)?;
    let path_tracing = path_tracing_audit(&config, &crowd_profile);
    let dlss = dlss_audit(&install, &config);
    let runtime = runtime_audit(
        args.telemetry.as_deref(),
        &args.runtime_thresholds,
        args.debug,
    )
    .map_err(|err| match err {
        runtime::RuntimeAuditError::RuntimeParse(message) => VerifyError::RuntimeParse(message),
        runtime::RuntimeAuditError::Input(message) => VerifyError::Input(message),
    })?;

    let mut sidecar = Sidecar {
        schema_version: SCHEMA_VERSION.to_string(),
        generated_at: Utc::now().to_rfc3339(),
        workload_profile: WORKLOAD_PROFILE.to_string(),
        game: discovery.game_section(),
        install,
        config,
        mods,
        crowd_profile,
        path_tracing,
        dlss,
        runtime,
        verdict: BTreeMap::new(),
        overall_status: OverallStatus::Warning,
        error_details: Vec::new(),
    };

    finalize_verdict(&mut sidecar);
    Ok(sidecar)
}

pub fn write_output(args: &Args, sidecar: &Sidecar) -> Result<(), VerifyError> {
    let json = serde_json::to_string_pretty(sidecar)
        .map_err(|err| VerifyError::Internal(format!("failed to serialize sidecar: {err}")))?;
    let text = render::render_text_summary(sidecar);

    if matches!(args.format, cli::OutputFormat::Text) {
        println!("{text}");
    }

    match (&args.out, args.format) {
        (Some(out), _) => {
            if args.dry_run {
                return Err(VerifyError::Usage(
                    "--dry-run requires stdout output; remove --out".to_string(),
                ));
            }
            let mut file = File::create(out).map_err(|err| {
                VerifyError::Input(format!(
                    "failed to create output file {}: {err}",
                    out.display()
                ))
            })?;
            file.write_all(json.as_bytes()).map_err(|err| {
                VerifyError::Input(format!(
                    "failed to write output file {}: {err}",
                    out.display()
                ))
            })?;
        }
        (None, cli::OutputFormat::Json) => {
            println!("{json}");
        }
        (None, cli::OutputFormat::Text) => {}
    }

    Ok(())
}

pub fn success_exit_code(sidecar: &Sidecar) -> i32 {
    if sidecar
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.runtime_parse_error.as_ref())
        .is_some()
    {
        3
    } else {
        0
    }
}
