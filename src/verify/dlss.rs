use crate::verify::types::{ClaimVerdict, ConfigSection, DlssSection};

pub fn dlss_audit(
    install: &crate::verify::types::InstallSection,
    config: &ConfigSection,
) -> DlssSection {
    let resolution_scaling = config
        .values
        .get("ResolutionScaling")
        .and_then(|value: &serde_json::Value| value.as_str())
        .map(ToOwned::to_owned);
    let backend_preset = config
        .values
        .get("DLSS_BackendPreset")
        .and_then(|value: &serde_json::Value| value.as_str())
        .map(ToOwned::to_owned);

    let all_binaries_present = install
        .required_binaries
        .values()
        .all(|binary| binary.verdict == ClaimVerdict::Pass);

    DlssSection {
        resolution_scaling: resolution_scaling.clone(),
        dlss_backend_preset: backend_preset.clone(),
        dlss: config
            .values
            .get("DLSS")
            .and_then(|value: &serde_json::Value| value.as_str())
            .map(ToOwned::to_owned),
        dlss_d: config
            .values
            .get("DLSS_D")
            .and_then(|value: &serde_json::Value| value.as_bool()),
        dlss_frame_gen: config
            .values
            .get("DLSSFrameGen")
            .and_then(|value: &serde_json::Value| value.as_bool()),
        dlss_multi_frame_generation: config
            .values
            .get("DLSS_MultiFrameGeneration")
            .and_then(|value: &serde_json::Value| value.as_str())
            .map(ToOwned::to_owned),
        dlss_4_download_verified: if all_binaries_present {
            ClaimVerdict::Pass
        } else {
            ClaimVerdict::Fail
        },
        dlss_transformer_selected: if backend_preset.as_deref() == Some("Transformer") {
            ClaimVerdict::Pass
        } else {
            ClaimVerdict::Fail
        },
        dlss_upscaling_enabled: if resolution_scaling.as_deref() == Some("DLSS") {
            ClaimVerdict::Pass
        } else {
            ClaimVerdict::Fail
        },
    }
}
