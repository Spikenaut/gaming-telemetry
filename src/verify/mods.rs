use std::collections::BTreeMap;
use std::path::Path;

use crate::verify::VerifyError;
use crate::verify::discovery::DiscoveryContext;
use crate::verify::fs::{canonical_is_within, dir_has_regular_files, evidence};
use crate::verify::types::{ClaimVerdict, ModArtifact, ModState, ModsSection};

struct ArtifactRule {
    name: &'static str,
    live_paths: &'static [&'static str],
    download_markers: &'static [&'static str],
    nonzero_paths: &'static [&'static str],
}

const ARTIFACTS: &[ArtifactRule] = &[
    ArtifactRule {
        name: "cet_active",
        live_paths: &[
            "bin/x64/plugins/cyber_engine_tweaks.asi",
            "bin/x64/plugins/cyber_engine_tweaks",
        ],
        download_markers: &["CET 1.37.1 - Scripting fixes-107-1-37-1-1759193708.zip"],
        nonzero_paths: &["bin/x64/plugins/cyber_engine_tweaks.asi"],
    },
    ArtifactRule {
        name: "ultra_plus_active",
        live_paths: &[
            "r6/scripts/UltraPlus.reds",
            "red4ext/plugins/UltraTool/UltraTool.dll",
            "bin/x64/plugins/cyber_engine_tweaks/mods/UltraPlus",
        ],
        download_markers: &["Cyberpunk Ultra Plus v8.4.1-10490-8-4-1-1776525190.zip"],
        nonzero_paths: &[
            "r6/scripts/UltraPlus.reds",
            "red4ext/plugins/UltraTool/UltraTool.dll",
        ],
    },
    ArtifactRule {
        name: "hd_textures_active",
        live_paths: &["archive/pc/mod/HD Reworked Project.archive"],
        download_markers: &["Cyberpunk 2077 HD Reworked Project Balanced-7652-2-0-1696952040.zip"],
        nonzero_paths: &["archive/pc/mod/HD Reworked Project.archive"],
    },
    ArtifactRule {
        name: "fake_pt_light_fix_active",
        live_paths: &["archive/pc/mod/UltraRemoveFakePTLights.xl"],
        download_markers: &["Cyberpunk Ultra Plus v8.4.1-10490-8-4-1-1776525190.zip"],
        nonzero_paths: &["archive/pc/mod/UltraRemoveFakePTLights.xl"],
    },
    ArtifactRule {
        name: "nova_crowds_active",
        live_paths: &[
            "bin/x64/plugins/cyber_engine_tweaks/mods/NovaCrowds/init.lua",
            "bin/x64/plugins/cyber_engine_tweaks/mods/NovaCrowds/db.sqlite3",
        ],
        download_markers: &["Nova Crowds-14211-1-0-7-1716378738.zip"],
        nonzero_paths: &["bin/x64/plugins/cyber_engine_tweaks/mods/NovaCrowds/init.lua"],
    },
    ArtifactRule {
        name: "dlss_enabler_active",
        live_paths: &["version.dll"],
        download_markers: &["DLSS Enabler 4.6.0 STABLE-757-4-6-0-1776280563.zip"],
        nonzero_paths: &["version.dll"],
    },
];

pub fn mods_audit(ctx: &DiscoveryContext, debug: bool) -> Result<ModsSection, VerifyError> {
    let game_root = ctx
        .game_root
        .as_ref()
        .ok_or_else(|| VerifyError::Input("game root not discovered".to_string()))?;
    let mut artifacts = BTreeMap::new();
    let backup_root = crate::verify::fs::expand_home("~/mod-backups");

    for artifact in ARTIFACTS {
        let mut evidence_list = Vec::new();
        let mut forensic = Vec::new();
        let mut live_hits = 0usize;
        let mut has_error = false;

        for relative in artifact.live_paths {
            let path = game_root.join(relative);
            if path.exists() {
                let file = evidence(&path, "game_root");
                if file.is_symlink && !canonical_is_within(&path, game_root) {
                    forensic.push(file.clone());
                    continue;
                }
                if !file.readable {
                    has_error = true;
                }
                if path.is_dir() {
                    if dir_has_regular_files(&path) {
                        live_hits += 1;
                    }
                } else if !artifact.nonzero_paths.contains(relative) || file.size_bytes > 0 {
                    live_hits += 1;
                }
                evidence_list.push(file);
            }
        }

        if backup_root.exists() {
            for relative in artifact.live_paths {
                let name = Path::new(relative)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if name.is_empty() {
                    continue;
                }
                for entry in walkdir::WalkDir::new(&backup_root)
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter(|entry| entry.file_name().to_string_lossy() == name)
                {
                    forensic.push(evidence(entry.path(), "backup"));
                }
            }
        }

        let downloaded_only = artifact
            .download_markers
            .iter()
            .map(|marker| game_root.join(marker))
            .find(|path| path.exists())
            .map(|path| evidence(&path, "download"));
        if let Some(download) = downloaded_only.clone() {
            forensic.push(download);
        }

        let state = if live_hits == artifact.live_paths.len() {
            ModState::ActiveInstalled
        } else if live_hits > 0 || !forensic.is_empty() {
            ModState::PartiallyInstalledBroken
        } else if downloaded_only.is_some() {
            ModState::DownloadedOnly
        } else {
            ModState::Missing
        };

        let verdict = if has_error {
            ClaimVerdict::Error
        } else if matches!(state, ModState::ActiveInstalled) {
            ClaimVerdict::Pass
        } else {
            ClaimVerdict::Fail
        };

        if debug {
            eprintln!(
                "[verify_cyberpunk][debug] mod {} state={:?} verdict={:?}",
                artifact.name, state, verdict
            );
        }

        artifacts.insert(
            artifact.name.to_string(),
            ModArtifact {
                state,
                verdict,
                evidence: evidence_list,
                forensic_evidence: forensic,
            },
        );
    }

    Ok(ModsSection { artifacts })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn backup_symlink_is_not_active() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path();
        unsafe {
            std::env::set_var("HOME", home);
        }
        let game_root = home.join(".local/share/Steam/steamapps/common/Cyberpunk 2077");
        std::fs::create_dir_all(game_root.join("bin/x64/plugins")).unwrap();
        let backup = home.join("mod-backups/cyberpunk-2026-04-18/cyber_engine_tweaks.asi");
        std::fs::create_dir_all(backup.parent().unwrap()).unwrap();
        std::fs::write(&backup, b"backup").unwrap();
        symlink(
            &backup,
            game_root.join("bin/x64/plugins/cyber_engine_tweaks.asi"),
        )
        .unwrap();

        let ctx = DiscoveryContext {
            search_paths: vec![],
            game_root: Some(game_root),
            manifest: None,
            proton_prefix: None,
            source_label: Some("test".to_string()),
        };
        let mods = mods_audit(&ctx, false).unwrap();
        assert_eq!(
            mods.artifacts["cet_active"].state,
            ModState::PartiallyInstalledBroken
        );
    }
}
