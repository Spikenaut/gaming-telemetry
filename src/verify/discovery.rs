use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::verify::VerifyError;
use crate::verify::acf::parse_manifest;
use crate::verify::fs::{evidence, expand_home};
use crate::verify::types::{BinaryEvidence, FileEvidence, GameSection, InstallSection};

const REQUIRED_DLLS: &[&str] = &[
    "nvngx_dlss.dll",
    "nvngx_dlssd.dll",
    "nvngx_dlssg.dll",
    "sl.common.dll",
    "sl.dlss.dll",
    "sl.dlss_d.dll",
    "sl.dlss_g.dll",
    "sl.interposer.dll",
    "sl.reflex.dll",
];

#[derive(Debug, Clone)]
pub struct DiscoveryContext {
    pub search_paths: Vec<FileEvidence>,
    pub game_root: Option<PathBuf>,
    pub manifest: Option<PathBuf>,
    pub proton_prefix: Option<PathBuf>,
    pub source_label: Option<String>,
}

pub fn discover_environment(debug: bool) -> Result<DiscoveryContext, VerifyError> {
    let candidates = vec![
        (
            "steam_local",
            expand_home("~/.local/share/Steam/steamapps/common/Cyberpunk 2077"),
            Some(expand_home(
                "~/.local/share/Steam/steamapps/appmanifest_1091500.acf",
            )),
            Some(expand_home(
                "~/.local/share/Steam/steamapps/compatdata/1091500/pfx/drive_c/users/steamuser",
            )),
        ),
        (
            "steam_legacy",
            expand_home("~/.steam/steam/steamapps/common/Cyberpunk 2077"),
            Some(expand_home(
                "~/.steam/steam/steamapps/appmanifest_1091500.acf",
            )),
            None,
        ),
        (
            "proton_prefix",
            expand_home("~/.local/share/Steam/steamapps/common/Cyberpunk 2077"),
            None,
            Some(expand_home(
                "~/.local/share/Steam/steamapps/compatdata/1091500/pfx/drive_c/users/steamuser",
            )),
        ),
        (
            "flatpak",
            expand_home(
                "~/.var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/common/Cyberpunk 2077",
            ),
            Some(expand_home(
                "~/.var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/appmanifest_1091500.acf",
            )),
            Some(expand_home(
                "~/.var/app/com.valvesoftware.Steam/.local/share/Steam/steamapps/compatdata/1091500/pfx/drive_c/users/steamuser",
            )),
        ),
    ];

    let mut records = Vec::new();
    let mut selected_root = None;
    let mut selected_manifest = None;
    let mut selected_proton = None;
    let mut selected_label = None;

    for (label, game_root, manifest, proton_prefix) in candidates {
        records.push(evidence(&game_root, label));
        if let Some(manifest) = &manifest {
            records.push(evidence(manifest, label));
        }
        if let Some(proton_prefix) = &proton_prefix {
            records.push(evidence(proton_prefix, label));
        }

        if selected_root.is_none() && game_root.exists() {
            selected_root = Some(game_root.clone());
            selected_manifest = manifest.clone().filter(|p| p.exists());
            selected_proton = proton_prefix.clone().filter(|p| p.exists());
            selected_label = Some(label.to_string());
        } else if selected_proton.is_none() {
            selected_proton = proton_prefix.clone().filter(|p| p.exists());
        }

        if debug {
            eprintln!(
                "[verify_cyberpunk][debug] checked source={label} root={} manifest={} proton={}",
                game_root.display(),
                manifest
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "-".to_string()),
                proton_prefix
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "-".to_string())
            );
        }
    }

    if selected_root.is_none() {
        return Err(VerifyError::Input(
            "unable to discover Cyberpunk 2077 game root in the configured search order"
                .to_string(),
        ));
    }

    Ok(DiscoveryContext {
        search_paths: records,
        game_root: selected_root,
        manifest: selected_manifest,
        proton_prefix: selected_proton,
        source_label: selected_label,
    })
}

impl DiscoveryContext {
    pub fn game_section(&self) -> GameSection {
        GameSection {
            search_paths: self.search_paths.clone(),
            selected_game_root: self
                .game_root
                .as_ref()
                .map(|p| evidence(p, self.source_label.as_deref().unwrap_or("selected"))),
            selected_manifest: self
                .manifest
                .as_ref()
                .map(|p| evidence(p, self.source_label.as_deref().unwrap_or("selected"))),
            selected_proton_prefix: self
                .proton_prefix
                .as_ref()
                .map(|p| evidence(p, self.source_label.as_deref().unwrap_or("selected"))),
        }
    }
}

pub fn install_audit(ctx: &DiscoveryContext, debug: bool) -> Result<InstallSection, VerifyError> {
    let manifest = ctx
        .manifest
        .as_ref()
        .map(|path| {
            std::fs::read_to_string(path)
                .map_err(|err| {
                    VerifyError::Input(format!("failed to read manifest {}: {err}", path.display()))
                })
                .and_then(|content| parse_manifest(&content).map_err(VerifyError::Input))
        })
        .transpose()?;

    let mut required_binaries = BTreeMap::new();
    let game_root = ctx
        .game_root
        .as_ref()
        .ok_or_else(|| VerifyError::Input("game root not discovered".to_string()))?;
    let bin_root = game_root.join("bin/x64");

    for dll in REQUIRED_DLLS {
        let path = bin_root.join(dll);
        let file = path
            .exists()
            .then(|| evidence(&path, ctx.source_label.as_deref().unwrap_or("selected")));
        let verdict = match &file {
            Some(file) if file.readable && file.size_bytes > 1024 => {
                crate::verify::types::ClaimVerdict::Pass
            }
            Some(file) if !file.readable => crate::verify::types::ClaimVerdict::Error,
            _ => crate::verify::types::ClaimVerdict::Fail,
        };
        if debug {
            eprintln!(
                "[verify_cyberpunk][debug] install evidence {} -> {:?}",
                path.display(),
                verdict
            );
        }
        required_binaries.insert((*dll).to_string(), BinaryEvidence { file, verdict });
    }

    Ok(InstallSection {
        install_root: Some(game_root.display().to_string()),
        steam_buildid: manifest.as_ref().and_then(|m| m.steam_buildid.clone()),
        last_updated: manifest.as_ref().and_then(|m| m.last_updated.clone()),
        last_played: manifest.as_ref().and_then(|m| m.last_played.clone()),
        manifest: ctx
            .manifest
            .as_ref()
            .map(|p| evidence(p, ctx.source_label.as_deref().unwrap_or("selected"))),
        required_binaries,
    })
}
