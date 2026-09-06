// SPDX-License-Identifier: MIT OR Apache-2.0

//! Optional crash/error reporting, behind the `sentry` cargo feature.
//!
//! The collector is a local 5 ms poll daemon; a capture run must not need an
//! HTTP/TLS stack to write Parquet. With the feature off, every call here is a
//! no-op and the `sentry` dependency (and its `reqwest`/`native-tls` tree) is
//! not compiled at all. Callers stay free of `#[cfg]` either way.

/// Reads an environment variable, treating whitespace-only as unset.
fn env_nonempty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Release name reported alongside an event.
///
/// `SENTRY_RELEASE` wins so CI can pin the name it created; otherwise the build's
/// git SHA produces `gaming-telemetry@<sha>`, matching the release workflow.
pub fn resolve_release(git_sha: &str) -> String {
    if let Some(release) = env_nonempty("SENTRY_RELEASE") {
        return release;
    }

    let git_sha = git_sha.trim();
    if !git_sha.is_empty() && git_sha != "unknown" {
        return format!("gaming-telemetry@{git_sha}");
    }

    // No SHA available (e.g. a build from a source tarball). Avoid "@unknown",
    // which would not match any release the Sentry workflow creates.
    "gaming-telemetry".to_owned()
}

#[cfg(feature = "sentry")]
mod imp {
    use super::{env_nonempty, resolve_release};
    use crate::build_info::git_sha;
    use std::borrow::Cow;

    /// Keeps the reporting client alive; dropping it flushes pending events.
    pub struct Guard(Option<sentry::ClientInitGuard>);

    impl Guard {
        /// True when a client was configured and events will be sent.
        pub fn is_active(&self) -> bool {
            self.0.is_some()
        }
    }

    /// Captures the ambient scope so a `spawn_blocking` write reports under it.
    pub struct Scope(std::sync::Arc<sentry::Hub>);

    /// `SENTRY_DSN` is the client-side ingest key. It is deliberately NOT
    /// `SENTRY_AUTH_TOKEN`: that name means an org-scoped API token to
    /// `sentry-cli` in the release workflow, and must never reach a collector
    /// host's environment.
    pub fn init() -> Guard {
        init_with(
            env_nonempty("SENTRY_DSN"),
            env_nonempty("SENTRY_ENVIRONMENT"),
        )
    }

    /// Arming rules, separated from environment lookup so they can be tested
    /// without mutating process env.
    fn init_with(dsn: Option<String>, environment: Option<String>) -> Guard {
        let Some(dsn) = dsn else {
            return Guard(None);
        };
        let parsed_dsn = match dsn.parse() {
            Ok(parsed) => parsed,
            Err(error) => {
                eprintln!("Sentry disabled: invalid SENTRY_DSN ({error})");
                return Guard(None);
            }
        };

        // `ClientOptions` is #[non_exhaustive], so it must be built by mutation
        // rather than a struct literal. The defaults already match what a
        // collector wants: all events sampled, tracing disabled.
        let mut options = sentry::ClientOptions::default();
        options.dsn = Some(parsed_dsn);
        options.release = Some(Cow::Owned(resolve_release(&git_sha())));
        options.environment = Some(Cow::Owned(
            environment.unwrap_or_else(|| "local".to_owned()),
        ));

        Guard(Some(sentry::init(options)))
    }

    pub fn current_scope() -> Scope {
        Scope(sentry::Hub::current())
    }

    pub fn run_in_scope<T>(scope: Scope, f: impl FnOnce() -> T) -> T {
        sentry::Hub::run(scope.0, f)
    }

    pub fn capture_error(message: &str) {
        sentry::capture_message(message, sentry::Level::Error);
    }

    #[cfg(test)]
    mod tests {
        use super::init_with;

        #[test]
        fn arms_only_with_a_parseable_dsn() {
            assert!(!init_with(None, None).is_active(), "no DSN must not arm");
            assert!(
                !init_with(Some("not-a-dsn".to_owned()), None).is_active(),
                "an unparseable DSN must not arm"
            );
            assert!(
                init_with(
                    Some("https://key@00000.ingest.sentry.io/0".to_owned()),
                    Some("ci-test".to_owned()),
                )
                .is_active()
            );
        }
    }
}

#[cfg(not(feature = "sentry"))]
mod imp {
    /// No-op stand-in so callers need no `#[cfg]`.
    pub struct Guard;

    impl Guard {
        /// Always false: reporting is compiled out.
        pub fn is_active(&self) -> bool {
            false
        }
    }

    /// No-op stand-in so callers need no `#[cfg]`.
    pub struct Scope;

    pub fn init() -> Guard {
        Guard
    }

    pub fn current_scope() -> Scope {
        Scope
    }

    pub fn run_in_scope<T>(_scope: Scope, f: impl FnOnce() -> T) -> T {
        f()
    }

    pub fn capture_error(_message: &str) {}
}

pub use imp::{Guard, Scope, capture_error, current_scope, init, run_in_scope};

#[cfg(test)]
mod tests {
    use super::resolve_release;

    #[test]
    fn release_prefers_git_sha_when_env_unset() {
        assert_eq!(resolve_release("abc1234"), "gaming-telemetry@abc1234");
    }

    #[test]
    fn release_falls_back_when_sha_unknown() {
        assert_eq!(resolve_release("unknown"), "gaming-telemetry");
        assert_eq!(resolve_release("   "), "gaming-telemetry");
    }
}
