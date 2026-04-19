use crate::verify::types::Sidecar;

pub fn render_text_summary(sidecar: &Sidecar) -> String {
    let runtime = sidecar
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.runtime_corroboration)
        .map(|verdict| format!(" runtime={verdict:?}"))
        .unwrap_or_default();

    format!(
        "workload={} overall_status={:?} path_tracing={:?} dlss_transformer={:?} dlss_upscaling={:?} high_crowd={:?}{}",
        sidecar.workload_profile,
        sidecar.overall_status,
        sidecar.path_tracing.verdict,
        sidecar.dlss.dlss_transformer_selected,
        sidecar.dlss.dlss_upscaling_enabled,
        sidecar.crowd_profile.high_crowd_profile,
        runtime,
    )
}
