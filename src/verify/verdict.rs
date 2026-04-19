use crate::verify::types::{ClaimVerdict, OverallStatus, Sidecar};

pub fn finalize_verdict(sidecar: &mut Sidecar) {
    sidecar.verdict.insert(
        "dlss_4_download_verified".to_string(),
        sidecar.dlss.dlss_4_download_verified,
    );
    sidecar.verdict.insert(
        "dlss_transformer_selected".to_string(),
        sidecar.dlss.dlss_transformer_selected,
    );
    sidecar.verdict.insert(
        "dlss_upscaling_enabled".to_string(),
        sidecar.dlss.dlss_upscaling_enabled,
    );
    sidecar.verdict.insert(
        "path_tracing_enabled".to_string(),
        sidecar.path_tracing.verdict,
    );
    sidecar.verdict.insert(
        "high_crowd_profile".to_string(),
        sidecar.crowd_profile.high_crowd_profile,
    );

    for (name, artifact) in &sidecar.mods.artifacts {
        sidecar.verdict.insert(name.clone(), artifact.verdict);
    }
    if let Some(runtime) = &sidecar.runtime {
        if let Some(verdict) = runtime.runtime_corroboration {
            sidecar
                .verdict
                .insert("runtime_corroboration".to_string(), verdict);
        }
    }

    let required = [
        sidecar.dlss.dlss_4_download_verified,
        sidecar.dlss.dlss_transformer_selected,
        sidecar.dlss.dlss_upscaling_enabled,
        sidecar.path_tracing.verdict,
        sidecar.mods.artifacts["ultra_plus_active"].verdict,
        sidecar.mods.artifacts["hd_textures_active"].verdict,
        sidecar.mods.artifacts["nova_crowds_active"].verdict,
        sidecar.crowd_profile.high_crowd_profile,
    ];

    let runtime_required = sidecar
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.runtime_corroboration);

    let has_error = sidecar
        .verdict
        .values()
        .any(|verdict| *verdict == ClaimVerdict::Error);
    let all_required_pass = required
        .iter()
        .all(|verdict| *verdict == ClaimVerdict::Pass)
        && runtime_required
            .map(|v| v == ClaimVerdict::Pass)
            .unwrap_or(true);

    sidecar.overall_status = if has_error {
        OverallStatus::Error
    } else if all_required_pass {
        OverallStatus::Pass
    } else {
        OverallStatus::Warning
    };
}
