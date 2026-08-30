use super::*;

fn temp_dir() -> PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/test-fixtures")
        .join(format!(
            "manifest_rename_failure_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn temp_files(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "tmp"))
        .collect()
}

fn manifest() -> SessionManifest {
    SessionManifest::new(
        "s1".to_owned(),
        "re4r".to_owned(),
        Utc::now(),
        5,
        HostInfo::new(None, None),
    )
}

#[test]
fn write_atomic_leaves_parseable_json_and_no_temp_file() {
    let dir = temp_dir();
    manifest().write_atomic(&dir).unwrap();

    assert!(dir.join(MANIFEST_FILENAME).is_file());
    assert!(temp_files(&dir).is_empty());
    assert_eq!(
        SessionManifest::load(&dir).unwrap().unwrap().session_id,
        "s1"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn write_atomic_removes_temp_file_when_rename_fails() {
    let dir = temp_dir();
    std::fs::create_dir(dir.join(MANIFEST_FILENAME)).unwrap();

    assert!(manifest().write_atomic(&dir).is_err());
    assert!(temp_files(&dir).is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}
