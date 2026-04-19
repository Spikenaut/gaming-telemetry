use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
}

fn copy_tree(src: &Path, dst: &Path) {
    for entry in walkdir::WalkDir::new(src)
        .into_iter()
        .filter_map(Result::ok)
    {
        let rel = entry.path().strip_prefix(src).unwrap();
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).unwrap();
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

fn write_required_dlls(game_root: &Path) {
    let bin = game_root.join("bin/x64");
    fs::create_dir_all(&bin).unwrap();
    for dll in [
        "nvngx_dlss.dll",
        "nvngx_dlssd.dll",
        "nvngx_dlssg.dll",
        "sl.common.dll",
        "sl.dlss.dll",
        "sl.dlss_d.dll",
        "sl.dlss_g.dll",
        "sl.interposer.dll",
        "sl.reflex.dll",
    ] {
        fs::write(bin.join(dll), vec![b'X'; 2048]).unwrap();
    }
}

fn prepare_home(mod_fixture: &str, user_settings: &str) -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();
    let game_root = home.join(".local/share/Steam/steamapps/common/Cyberpunk 2077");
    let proton_root = home.join(".local/share/Steam/steamapps/compatdata/1091500/pfx/drive_c/users/steamuser/AppData/Local/CD Projekt Red/Cyberpunk 2077");
    fs::create_dir_all(&game_root).unwrap();
    fs::create_dir_all(&proton_root).unwrap();
    copy_tree(&fixture(mod_fixture), &game_root);
    write_required_dlls(&game_root);
    fs::write(
        home.join(".local/share/Steam/steamapps/appmanifest_1091500.acf"),
        fs::read_to_string(fixture("tests/fixtures/appmanifest_1091500.acf")).unwrap(),
    )
    .unwrap();
    fs::write(proton_root.join("UserSettings.json"), user_settings).unwrap();
    temp
}

#[test]
fn current_like_warning_case() {
    let user_settings =
        fs::read_to_string(fixture("tests/fixtures/proton/UserSettings.json")).unwrap();
    let home = prepare_home("tests/fixtures/mods/warning", &user_settings);

    let output = Command::new(env!("CARGO_BIN_EXE_verify_cyberpunk"))
        .env("HOME", home.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["overall_status"], "warning");
    assert_eq!(json["path_tracing"]["verdict"], "fail");
    assert_eq!(json["dlss"]["dlss_upscaling_enabled"], "fail");
}

#[test]
fn passing_case_with_runtime() {
    let warning = fs::read_to_string(fixture("tests/fixtures/proton/UserSettings.json")).unwrap();
    let pass_settings = warning
        .replace("\"FSR2\"", "\"DLSS\"")
        .replace(
            "\"RayTracedPathTracing\", \"value\": false",
            "\"RayTracedPathTracing\", \"value\": true",
        )
        .replace(
            "\"DLSS_D\", \"value\": false",
            "\"DLSS_D\", \"value\": true",
        )
        .replace(
            "\"DLSSFrameGen\", \"value\": false",
            "\"DLSSFrameGen\", \"value\": true",
        );
    let home = prepare_home("tests/fixtures/mods/pass", &pass_settings);

    let output = Command::new(env!("CARGO_BIN_EXE_verify_cyberpunk"))
        .env("HOME", home.path())
        .arg("--telemetry")
        .arg(fixture("tests/fixtures/runtime/pass.csv"))
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["overall_status"], "pass");
    assert_eq!(json["runtime"]["runtime_corroboration"], "pass");
}

#[test]
fn runtime_parse_error_returns_exit_3() {
    let user_settings =
        fs::read_to_string(fixture("tests/fixtures/proton/UserSettings.json")).unwrap();
    let home = prepare_home("tests/fixtures/mods/pass", &user_settings);
    let bad_csv = home.path().join("bad.csv");
    fs::write(&bad_csv, "not,a,telemetry\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_verify_cyberpunk"))
        .env("HOME", home.path())
        .arg("--telemetry")
        .arg(&bad_csv)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn missing_ultraplus_variables_is_not_hard_error() {
    let user_settings =
        fs::read_to_string(fixture("tests/fixtures/proton/UserSettings.json")).unwrap();
    let home = prepare_home("tests/fixtures/mods/broken", &user_settings);
    let variables = home.path().join(
        ".local/share/Steam/steamapps/common/Cyberpunk 2077/bin/x64/plugins/cyber_engine_tweaks/mods/UltraPlus/lib/Variables.lua",
    );
    if variables.exists() {
        fs::remove_file(&variables).unwrap();
    }

    let output = Command::new(env!("CARGO_BIN_EXE_verify_cyberpunk"))
        .env("HOME", home.path())
        .output()
        .unwrap();

    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["crowd_profile"]["settings_source"], "unknown");
}

#[cfg(unix)]
#[test]
fn unreadable_config_returns_exit_2() {
    use std::os::unix::fs::PermissionsExt;

    let user_settings =
        fs::read_to_string(fixture("tests/fixtures/proton/UserSettings.json")).unwrap();
    let home = prepare_home("tests/fixtures/mods/pass", &user_settings);
    let config = home.path().join(".local/share/Steam/steamapps/compatdata/1091500/pfx/drive_c/users/steamuser/AppData/Local/CD Projekt Red/Cyberpunk 2077/UserSettings.json");
    let mut permissions = fs::metadata(&config).unwrap().permissions();
    permissions.set_mode(0o000);
    fs::set_permissions(&config, permissions).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_verify_cyberpunk"))
        .env("HOME", home.path())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
}
