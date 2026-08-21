// SPDX-License-Identifier: MIT OR Apache-2.0

//! Build identity shared by Sentry release naming and the session manifest.

/// Collector version, from Cargo.
pub fn collector_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Short git SHA for this build.
///
/// Resolution order: `AGENTOS_GIT_SHA` (set by CI), then the source SHA embedded
/// by `build.rs`, then `"unknown"`. Runtime working directories never affect it.
pub fn git_sha() -> String {
    if let Some(value) = std::env::var("AGENTOS_GIT_SHA")
        .ok()
        .filter(|value| !value.trim().is_empty())
    {
        return value;
    }
    if let Some(sha) = option_env!("GAMING_TELEMETRY_GIT_SHA")
        .map(str::trim)
        .filter(|sha| !sha.is_empty())
    {
        return sha.to_owned();
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
