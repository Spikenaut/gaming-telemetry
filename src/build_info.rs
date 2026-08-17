// SPDX-License-Identifier: MIT OR Apache-2.0

//! Build identity shared by Sentry release naming and the session manifest.

use std::process::Command;

/// Collector version, from Cargo.
pub fn collector_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Short git SHA for this build.
///
/// Resolution order: `AGENTOS_GIT_SHA` (set by CI), then `git rev-parse --short
/// HEAD` for dev runs from a source checkout, then `"unknown"`.
///
/// Packaged binaries run from arbitrary working directories should set
/// `AGENTOS_GIT_SHA` (or `SENTRY_RELEASE`) so the value is not derived from
/// whatever directory the operator happened to launch from.
pub fn git_sha() -> String {
    if let Some(value) = std::env::var("AGENTOS_GIT_SHA")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return value;
    }
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        if output.status.success() {
            let sha = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if !sha.is_empty() {
                return sha;
            }
        }
    }
    "unknown".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collector_version_is_non_empty() {
        assert!(!collector_version().is_empty());
    }

    #[test]
    fn git_sha_never_returns_empty() {
        // Whatever branch is taken (env, git, fallback), the value is usable as a
        // manifest field.
        assert!(!git_sha().trim().is_empty());
    }
}
