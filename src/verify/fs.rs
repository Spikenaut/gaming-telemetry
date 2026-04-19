use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use crate::verify::types::FileEvidence;

pub fn expand_home(path: &str) -> PathBuf {
    if let Some(stripped) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    PathBuf::from(path)
}

pub fn evidence(path: &Path, source_label: &str) -> FileEvidence {
    let metadata = fs::symlink_metadata(path).ok();
    let readable = if metadata.as_ref().is_some_and(|m| m.is_dir()) {
        fs::read_dir(path).is_ok()
    } else {
        File::open(path).is_ok()
    };
    let mtime = metadata
        .as_ref()
        .and_then(|m| m.modified().ok())
        .map(|t| DateTime::<Utc>::from(t).to_rfc3339());
    FileEvidence {
        path: path.display().to_string(),
        source_label: source_label.to_string(),
        size_bytes: metadata.as_ref().map(|m| m.len()).unwrap_or(0),
        mtime,
        readable,
        is_symlink: metadata
            .as_ref()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false),
    }
}

pub fn dir_has_regular_files(path: &Path) -> bool {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .any(|entry| entry.file_type().is_file())
}

pub fn read_to_string(path: &Path) -> std::io::Result<String> {
    let mut file = File::open(path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;
    Ok(content)
}

pub fn canonical_is_within(path: &Path, root: &Path) -> bool {
    let path = path.canonicalize();
    let root = root.canonicalize();
    match (path, root) {
        (Ok(path), Ok(root)) => path.starts_with(root),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_home_paths() {
        let home = tempfile::tempdir().unwrap();
        unsafe {
            std::env::set_var("HOME", home.path());
        }
        let path = expand_home("~/example");
        assert!(path.starts_with(home.path()));
    }
}
