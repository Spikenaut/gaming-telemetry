use crate::verify::types::{ClaimVerdict, ConfigSection, CrowdProfileSection, PathTracingSection};

pub fn path_tracing_audit(
    config: &ConfigSection,
    crowd: &CrowdProfileSection,
) -> PathTracingSection {
    let ray_tracing = config
        .values
        .get("RayTracing")
        .and_then(|value: &serde_json::Value| value.as_bool());
    let path_tracing = config
        .values
        .get("RayTracedPathTracing")
        .and_then(|value: &serde_json::Value| value.as_bool());
    let photo_mode = config
        .values
        .get("RayTracedPathTracingForPhotoMode")
        .and_then(|value: &serde_json::Value| value.as_bool());

    let verdict = if ray_tracing == Some(true) && path_tracing == Some(true) {
        ClaimVerdict::Pass
    } else {
        ClaimVerdict::Fail
    };

    PathTracingSection {
        ray_tracing,
        ray_traced_path_tracing: path_tracing,
        ray_traced_path_tracing_for_photo_mode: photo_mode,
        ultra_plus_mode: crowd
            .ultra_plus_mode
            .clone()
            .unwrap_or_else(|| "unknown/default".to_string()),
        verdict,
    }
}
