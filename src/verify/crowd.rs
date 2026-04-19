use regex::Regex;

use crate::verify::discovery::DiscoveryContext;
use crate::verify::fs::read_to_string;
use crate::verify::types::{
    ClaimVerdict, ConfigSection, CrowdProfileSection, CrowdSettings, ModState, ModsSection,
    SettingsSource,
};
use crate::verify::VerifyError;

pub fn crowd_profile_audit(
    ctx: &DiscoveryContext,
    config: &ConfigSection,
    mods: &ModsSection,
    debug: bool,
) -> Result<CrowdProfileSection, VerifyError> {
    let game_root = ctx
        .game_root
        .as_ref()
        .ok_or_else(|| VerifyError::Input("game root not discovered".to_string()))?;

    let nova_path = game_root
        .join("bin/x64/plugins/cyber_engine_tweaks/mods/NovaCrowds/settings/settings.json");
    let nova_settings = if nova_path.exists() {
        let content = read_to_string(&nova_path).map_err(|err| {
            VerifyError::Input(format!("failed to read {}: {err}", nova_path.display()))
        })?;
        let json: serde_json::Value = serde_json::from_str(&content).map_err(|err| {
            VerifyError::Input(format!("failed to parse {}: {err}", nova_path.display()))
        })?;
        Some(CrowdSettings {
            multiplier: json.get("multiplier").and_then(|v| v.as_i64()),
            shuffle_amount: json.get("shuffleAmount").and_then(|v| v.as_i64()),
            disable_lq_crowds: json.get("disableLqCrowds").and_then(|v| v.as_bool()),
        })
    } else {
        None
    };

    let ultra_plus_config = find_ultraplus_config(game_root);
    let (settings_source, crowds_enabled, mode) = if let Some(config_path) = ultra_plus_config {
        let content = read_to_string(&config_path).map_err(|err| {
            VerifyError::Input(format!("failed to read {}: {err}", config_path.display()))
        })?;
        (
            SettingsSource::Persisted,
            parse_ini_bool(&content, "crowds"),
            parse_ini_string(&content, "mode").unwrap_or_else(|| "unknown/default".to_string()),
        )
    } else {
        let variables =
            game_root.join("bin/x64/plugins/cyber_engine_tweaks/mods/UltraPlus/lib/Variables.lua");
        if variables.exists() {
            let content = read_to_string(&variables).map_err(|err| {
                VerifyError::Input(format!("failed to read {}: {err}", variables.display()))
            })?;
            (
                SettingsSource::Defaults,
                parse_lua_default_crowds(&content),
                "unknown/default".to_string(),
            )
        } else {
            (SettingsSource::Unknown, None, "unknown/default".to_string())
        }
    };

    let base_density = config
        .values
        .get("/graphics/performance CrowdDensity")
        .and_then(|value: &serde_json::Value| value.as_str())
        .map(ToOwned::to_owned);
    let nova_active = mods
        .artifacts
        .get("nova_crowds_active")
        .map(|artifact| artifact.state == ModState::ActiveInstalled)
        .unwrap_or(false);
    let high_crowd_profile = base_density.as_deref() == Some("High")
        && nova_active
        && nova_settings
            .as_ref()
            .and_then(|s| s.multiplier)
            .unwrap_or(0)
            >= 1
        && nova_settings
            .as_ref()
            .and_then(|s| s.shuffle_amount)
            .unwrap_or(0)
            >= 2
        && nova_settings.as_ref().and_then(|s| s.disable_lq_crowds) == Some(false)
        && crowds_enabled == Some(true);

    if debug {
        eprintln!(
            "[verify_cyberpunk][debug] crowd base={:?} nova_active={} settings_source={:?} crowds_enabled={:?}",
            base_density, nova_active, settings_source, crowds_enabled
        );
    }

    Ok(CrowdProfileSection {
        base_crowd_density: base_density,
        high_crowd_profile: if high_crowd_profile {
            ClaimVerdict::Pass
        } else {
            ClaimVerdict::Fail
        },
        settings_source,
        ultra_plus_crowds_enabled: crowds_enabled,
        ultra_plus_mode: Some(mode),
        nova_crowds: nova_settings,
    })
}

fn find_ultraplus_config(game_root: &std::path::Path) -> Option<std::path::PathBuf> {
    walkdir::WalkDir::new(game_root)
        .max_depth(6)
        .into_iter()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy() == "UltraPlusConfig.ini")
        .map(|entry| entry.path().to_path_buf())
}

fn parse_ini_bool(input: &str, key: &str) -> Option<bool> {
    let pattern = format!(r"(?im)^\s*{}\s*=\s*(true|false)\s*$", regex::escape(key));
    let regex = Regex::new(&pattern).ok()?;
    let captures = regex.captures(input)?;
    Some(&captures[1].to_ascii_lowercase() == "true")
}

fn parse_ini_string(input: &str, key: &str) -> Option<String> {
    let pattern = format!(r"(?im)^\s*{}\s*=\s*(.+?)\s*$", regex::escape(key));
    let regex = Regex::new(&pattern).ok()?;
    let captures = regex.captures(input)?;
    Some(captures[1].trim().trim_matches('"').to_string())
}

pub fn parse_lua_default_crowds(input: &str) -> Option<bool> {
    let regex = Regex::new(r#"(?m)crowds\s*=\s*(true|false)"#).ok()?;
    let captures = regex.captures(input)?;
    Some(&captures[1].to_ascii_lowercase() == "true")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lua_crowd_default() {
        let input = "Var.settings = {\n crowds = true,\n}";
        assert_eq!(parse_lua_default_crowds(input), Some(true));
    }
}
