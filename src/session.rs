// SPDX-License-Identifier: MIT OR Apache-2.0

//! Session directory, label, and batch-numbering helpers.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Prefix of the Parquet batch files the collector writes.
pub const BATCH_PREFIX: &str = "gpu_telemetry_v2_batch_";
pub const BATCH_SUFFIX: &str = ".parquet";

/// Keep session tags as short ASCII identifiers (labels only — never used for
/// paths or exec).
pub fn sanitize_label(raw: &str) -> String {
    raw.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        .take(64)
        .collect()
}

/// Multi-game session tag. Production uses `SESSION_LABEL` only; the optional
/// `cli_label` parameter exists so precedence and sanitization stay unit testable.
pub fn resolve_label_from_sources(cli_label: Option<&str>, env_label: Option<&str>) -> String {
    let cli_filtered = cli_label.map(str::trim).filter(|value| !value.is_empty());
    let env_filtered = env_label.map(str::trim).filter(|value| !value.is_empty());
    sanitize_label(cli_filtered.or(env_filtered).unwrap_or(""))
}

/// Runtime label resolution. Operators set `SESSION_LABEL` (see README).
///
/// CLI argv is intentionally not read here: Codacy flags `std::env::args` as a
/// security surface for the long-running daemon, and env alone is enough for
/// multi-title capture.
pub fn resolve_label() -> String {
    resolve_label_from_sources(None, std::env::var("SESSION_LABEL").ok().as_deref())
}

/// Where this run writes its batches, manifest, and events.
///
/// `SESSION_DIR` is created if missing. Unset falls back to the current working
/// directory, which is how the collector behaved before session directories
/// existed.
pub fn resolve_dir() -> Result<PathBuf> {
    match std::env::var("SESSION_DIR")
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        Some(dir) => {
            let path = PathBuf::from(dir);
            std::fs::create_dir_all(&path).with_context(|| {
                format!(
                    "failed to create SESSION_DIR {}",
                    crate::privacy::redact_personal_path(&path.display().to_string())
                )
            })?;
            Ok(path)
        }
        None => std::env::current_dir().context("failed to read current directory"),
    }
}

/// Stable identifier for a capture session.
///
/// Derived from the session directory's own name so a directory is
/// self-identifying. Falls back to `<label>_<timestamp>` when the directory name is
/// unusable (empty, or nothing survives sanitization — e.g. a path ending in `/`,
/// or a CJK-only directory name).
pub fn session_id(dir: &Path, label: &str, started_at: &str) -> String {
    let from_dir = dir
        .file_name()
        .map(|name| sanitize_label(&name.to_string_lossy()))
        .unwrap_or_default();
    if !from_dir.is_empty() {
        return from_dir;
    }
    let stamp = sanitize_label(started_at);
    if label.is_empty() {
        format!("session_{stamp}")
    } else {
        format!("{label}_{stamp}")
    }
}

/// Highest batch id already present in `dir`, or 0 if there are none.
///
/// The collector used to start `batch_counter` at 0 on every process, so
/// restarting a capture in a populated directory silently overwrote
/// `gpu_telemetry_v2_batch_1.parquet`. Numbering resumes from here instead.
///
/// Parses the numeric suffix rather than sorting names — lexicographically
/// `batch_10` sorts before `batch_2`.
pub fn highest_batch_id(dir: &Path) -> u32 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            name.strip_prefix(BATCH_PREFIX)?
                .strip_suffix(BATCH_SUFFIX)?
                .parse::<u32>()
                .ok()
        })
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-fixtures")
            .join(format!(
                "session_{tag}_{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn sanitize_label_strips_shell_metacharacters() {
        assert_eq!(sanitize_label("kcd2;rm -rf /"), "kcd2rm-rf");
        assert_eq!(sanitize_label("re_requiem"), "re_requiem");
        assert_eq!(sanitize_label("...ok"), "...ok");
        assert_eq!(sanitize_label(&"a".repeat(80)).len(), 64);
    }

    #[test]
    fn resolve_label_precedence_and_trimming() {
        assert_eq!(resolve_label_from_sources(None, None), "");
        assert_eq!(resolve_label_from_sources(None, Some("")), "");
        assert_eq!(resolve_label_from_sources(None, Some("  kcd2  ")), "kcd2");
        assert_eq!(
            resolve_label_from_sources(Some("cli"), Some("env")),
            "cli",
            "CLI label should take precedence over env"
        );
        assert_eq!(
            resolve_label_from_sources(Some(""), Some("env")),
            "env",
            "empty CLI label should fall back to env"
        );
    }

    #[test]
    fn session_id_prefers_the_directory_name() {
        let id = session_id(Path::new("/data/kcd2_20260816_101500"), "kcd2", "stamp");
        assert_eq!(id, "kcd2_20260816_101500");
    }

    #[test]
    fn session_id_falls_back_when_directory_name_is_unusable() {
        // Nothing survives sanitization, so the directory cannot identify itself.
        let id = session_id(Path::new("/data/日本語"), "kcd2", "20260816T101500Z");
        assert_eq!(id, "kcd2_20260816T101500Z");

        // Root has no file_name at all, and with no label we still get something
        // non-empty rather than a bare timestamp.
        let id = session_id(Path::new("/"), "", "20260816T101500Z");
        assert_eq!(id, "session_20260816T101500Z");
    }

    #[test]
    fn highest_batch_id_is_zero_for_empty_and_missing_dirs() {
        let dir = temp_dir("empty");
        assert_eq!(highest_batch_id(&dir), 0);
        assert_eq!(highest_batch_id(Path::new("/nonexistent/path/xyz")), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn highest_batch_id_parses_numerically_not_lexicographically() {
        let dir = temp_dir("numeric");
        for id in [1u32, 2, 10] {
            std::fs::write(dir.join(format!("{BATCH_PREFIX}{id}{BATCH_SUFFIX}")), b"x").unwrap();
        }
        // Lexicographic ordering would answer "2" here.
        assert_eq!(highest_batch_id(&dir), 10);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn highest_batch_id_ignores_unrelated_files() {
        let dir = temp_dir("unrelated");
        std::fs::write(dir.join("session_manifest.json"), b"{}").unwrap();
        std::fs::write(dir.join("canonical.csv"), b"x").unwrap();
        std::fs::write(
            dir.join(format!("{BATCH_PREFIX}notanumber{BATCH_SUFFIX}")),
            b"x",
        )
        .unwrap();
        std::fs::write(dir.join(format!("{BATCH_PREFIX}7{BATCH_SUFFIX}")), b"x").unwrap();
        assert_eq!(highest_batch_id(&dir), 7);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
