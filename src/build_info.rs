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
    git_sha_from_sources(
        std::env::var("AGENTOS_GIT_SHA").ok().as_deref(),
        option_env!("GAMING_TELEMETRY_GIT_SHA"),
    )
}

fn git_sha_from_sources(agentos_sha: Option<&str>, embedded_sha: Option<&str>) -> String {
    [agentos_sha, embedded_sha]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|sha| !sha.is_empty())
        .unwrap_or("unknown")
        .to_owned()
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

    #[test]
    fn git_sha_trims_agentos_env_var() {
        assert_eq!(git_sha_from_sources(Some("  abc123  "), None), "abc123");
        assert_eq!(
            git_sha_from_sources(Some(" "), Some(" embedded ")),
            "embedded"
        );
    }
}
