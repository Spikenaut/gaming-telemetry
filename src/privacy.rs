// SPDX-License-Identifier: MIT OR Apache-2.0

//! Path redaction for anything the collector prints or records.
//!
//! Session directories and manifests are shared with downstream pipelines, and
//! error text reaches logs, so absolute paths are stripped of the operator's
//! identity first. The collector never walks `$HOME` / Steam / Proton — this is
//! about not echoing paths it was handed.

use std::env;
use std::path::Path;

/// Replace occurrences of `home` with the literal `$HOME`.
///
/// An empty or root `home` is refused: replacing `/` would rewrite every path in
/// the string into nonsense.
fn redact_with_home(text: &str, home: &str) -> String {
    if home.is_empty() || home == "/" {
        return text.to_string();
    }
    text.replace(home, "$HOME")
}

/// Replace whole path components equal to `user` with `$USER`.
///
/// Component-wise rather than substring: replacing bare occurrences would mangle
/// unrelated words that merely contain the name (`alice` inside `/opt/alicent`).
/// Very short names are skipped for the same reason, and `root` is left alone —
/// it identifies nobody and appears in legitimate system paths.
fn redact_user_components(text: &str, user: &str) -> String {
    if user.len() < 3 || user == "root" {
        return text.to_string();
    }
    text.split('/')
        .map(|component| {
            if component == user {
                "$USER"
            } else {
                component
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// The operator's login name, for stripping it out of path components.
///
/// Falls back to the last component of `$HOME`, so a process started without
/// `USER`/`LOGNAME` still redacts.
fn current_user() -> Option<String> {
    for key in ["USER", "LOGNAME"] {
        if let Some(value) = env::var_os(key) {
            let value = value.to_string_lossy().trim().to_owned();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    let home = env::var_os("HOME")?;
    Path::new(&home)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
}

/// Redact embedded occurrences of the user's `$HOME`.
///
/// Returns the text unchanged when `HOME` is unset; `redact_personal_path` is the
/// entry point that still strips the username in that case.
pub fn redact_home(text: &str) -> String {
    match env::var_os("HOME") {
        Some(home) => redact_with_home(text, home.to_string_lossy().as_ref()),
        None => text.to_string(),
    }
}

/// Strip the operator's identity from a path or an error message containing one.
///
/// Two passes, because `$HOME` alone is not enough. A `SESSION_DIR` on external
/// media (`/run/media/<user>/…`, `/media/<user>/…`) carries the username without
/// ever going through the home directory, and a unit started without `HOME` set
/// — systemd services and containers routinely are — would otherwise redact
/// nothing at all.
pub fn redact_personal_path(path: &str) -> String {
    let redacted = redact_home(path);
    match current_user() {
        Some(user) => redact_user_components(&redacted, &user),
        None => redacted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_home_replaces_prefix() {
        let home = "/home/testuser";
        let example = format!("{home}/neuromorphic_data/kcd2_20260907");
        let result = redact_with_home(&example, home);
        assert_eq!(result, "$HOME/neuromorphic_data/kcd2_20260907");
    }

    #[test]
    fn redact_with_home_skips_empty_and_root() {
        let input = "/some/path";
        assert_eq!(redact_with_home(input, ""), input);
        assert_eq!(
            redact_with_home(input, "/"),
            input,
            "replacing / would rewrite every path in the string"
        );
    }

    /// The case `$HOME` redaction alone misses: a session directory on external
    /// media never passes through the home directory.
    #[test]
    fn user_components_are_redacted_outside_home() {
        assert_eq!(
            redact_user_components("/run/media/alice/ssd/neuromorphic_data", "alice"),
            "/run/media/$USER/ssd/neuromorphic_data"
        );
        assert_eq!(
            redact_user_components("/media/alice/usb", "alice"),
            "/media/$USER/usb"
        );
    }

    /// Component-wise, so a word that merely contains the name survives intact.
    #[test]
    fn only_whole_path_components_are_replaced() {
        assert_eq!(
            redact_user_components("/opt/alicent/data", "alice"),
            "/opt/alicent/data",
            "a substring match must not be redacted"
        );
        assert_eq!(
            redact_user_components("failed to open alice.parquet", "alice"),
            "failed to open alice.parquet",
            "only path components, not arbitrary words"
        );
    }

    #[test]
    fn ambiguous_or_shared_names_are_left_alone() {
        assert_eq!(redact_user_components("/var/root/x", "root"), "/var/root/x");
        assert_eq!(redact_user_components("/a/b/c", "a"), "/a/b/c");
    }

    #[test]
    fn redaction_is_idempotent() {
        let already = "$HOME/neuromorphic_data";
        assert_eq!(redact_personal_path(already), already);
        let user_redacted = "/run/media/$USER/ssd";
        assert_eq!(
            redact_user_components(user_redacted, "alice"),
            user_redacted
        );
    }

    #[test]
    fn trailing_and_bare_components_are_handled() {
        assert_eq!(
            redact_user_components("/home/alice", "alice"),
            "/home/$USER"
        );
        assert_eq!(redact_user_components("alice", "alice"), "$USER");
    }
}
