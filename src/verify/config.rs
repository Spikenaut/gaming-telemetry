use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::verify::VerifyError;
use crate::verify::discovery::DiscoveryContext;
use crate::verify::fs::{evidence, read_to_string};
use crate::verify::types::ConfigSection;

const CONFIG_KEYS: &[(&str, &str)] = &[
    ("ResolutionScaling", "ResolutionScaling"),
    ("DLSS", "DLSS"),
    ("DLSS_BackendPreset", "DLSS_BackendPreset"),
    ("DLSS_D", "DLSS_D"),
    ("FrameGeneration", "FrameGeneration"),
    ("DLSS_MultiFrameGeneration", "DLSS_MultiFrameGeneration"),
    ("DLSSFrameGen", "DLSSFrameGen"),
    ("RayTracing", "RayTracing"),
    ("RayTracedPathTracing", "RayTracedPathTracing"),
    (
        "RayTracedPathTracingForPhotoMode",
        "RayTracedPathTracingForPhotoMode",
    ),
    ("RayTracedLighting", "RayTracedLighting"),
    ("ReflexMode", "ReflexMode"),
    ("/graphics/performance CrowdDensity", "CrowdDensity"),
];

pub fn config_audit(ctx: &DiscoveryContext, debug: bool) -> Result<ConfigSection, VerifyError> {
    let candidates = config_candidates(ctx);
    let (path, _source, confidence) = candidates
        .into_iter()
        .find(|(path, _, _)| path.exists())
        .ok_or_else(|| VerifyError::Input("unable to discover UserSettings.json".to_string()))?;

    let content = read_to_string(&path).map_err(|err| {
        VerifyError::Input(format!("failed to read config {}: {err}", path.display()))
    })?;
    let json: serde_json::Value = serde_json::from_str(&content).map_err(|err| {
        VerifyError::Input(format!("failed to parse config {}: {err}", path.display()))
    })?;

    let mut values = BTreeMap::new();
    for (output_key, lookup_key) in CONFIG_KEYS {
        if let Some(value) = extract_option(&json, lookup_key) {
            values.insert((*output_key).to_string(), value);
        }
    }

    if debug {
        eprintln!(
            "[verify_cyberpunk][debug] config source={} confidence={confidence}",
            path.display()
        );
    }

    Ok(ConfigSection {
        config_source: Some(path.display().to_string()),
        config_confidence: Some(confidence.to_string()),
        config_file: Some(evidence(&path, "config")),
        values,
    })
}

fn config_candidates(ctx: &DiscoveryContext) -> Vec<(PathBuf, &'static str, &'static str)> {
    let mut candidates = Vec::new();
    if let Some(prefix) = &ctx.proton_prefix {
        candidates.push((
            prefix.join("AppData/Local/CD Projekt Red/Cyberpunk 2077/UserSettings.json"),
            "proton_user_settings",
            "high",
        ));
    }

    for root in [
        crate::verify::fs::expand_home("~/.local/share/Steam/userdata"),
        crate::verify::fs::expand_home("~/.steam/steam/userdata"),
        crate::verify::fs::expand_home(
            "~/.var/app/com.valvesoftware.Steam/.local/share/Steam/userdata",
        ),
    ] {
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                candidates.push((
                    entry.path().join("1091500/local/UserSettings.json"),
                    "steam_userdata",
                    "medium",
                ));
            }
        }
    }

    if let Some(game_root) = &ctx.game_root {
        candidates.push((
            game_root.join("UserSettings.json"),
            "game_root_fallback",
            "low",
        ));
    }

    candidates
}

fn extract_option(json: &serde_json::Value, key: &str) -> Option<serde_json::Value> {
    let groups = json.get("data")?.as_array()?;
    let prefer_graphics = key == "CrowdDensity";

    for group in groups {
        let group_name = group
            .get("group_name")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if prefer_graphics && group_name != "/graphics/performance" {
            continue;
        }
        for option in group.get("options")?.as_array()? {
            if option.get("name")?.as_str()? == key {
                return option.get("value").cloned();
            }
        }
    }

    if prefer_graphics {
        for group in groups {
            for option in group.get("options")?.as_array()? {
                if option.get("name")?.as_str()? == key {
                    return option.get("value").cloned();
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_known_keys() {
        let json: serde_json::Value = serde_json::from_str(
            r#"{"data":[{"group_name":"/graphics/performance","options":[{"name":"CrowdDensity","value":"High"}]},{"group_name":"/graphics/raytracing","options":[{"name":"RayTracing","value":true}]}]}"#,
        )
        .unwrap();
        assert_eq!(
            extract_option(&json, "CrowdDensity").unwrap(),
            serde_json::json!("High")
        );
        assert_eq!(
            extract_option(&json, "RayTracing").unwrap(),
            serde_json::json!(true)
        );
    }
}
