use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=AGENTOS_GIT_SHA");
    if let Ok(head_path) = git_path("HEAD") {
        println!("cargo:rerun-if-changed={head_path}");
        if let Ok(head) = std::fs::read_to_string(&head_path) {
            if let Some(reference) = head.strip_prefix("ref: ").map(str::trim) {
                if let Ok(reference_path) = git_path(reference) {
                    println!("cargo:rerun-if-changed={reference_path}");
                }
                if let Ok(packed_refs) = git_path("packed-refs") {
                    println!("cargo:rerun-if-changed={packed_refs}");
                }
            }
        }
    }

    let sha = std::env::var("AGENTOS_GIT_SHA")
        .ok()
        .filter(|sha| !sha.trim().is_empty())
        .or_else(|| {
            Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .map(|sha| sha.trim().to_owned())
                .filter(|sha| !sha.is_empty())
        })
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=GAMING_TELEMETRY_GIT_SHA={sha}");
}

fn git_path(path: &str) -> Result<String, ()> {
    Command::new("git")
        .args(["rev-parse", "--git-path", path])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|path| path.trim().to_owned())
        .filter(|path| !path.is_empty())
        .ok_or(())
}
