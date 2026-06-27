use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;

#[test]
fn help_mentions_profile_commands() {
    let mut cmd = Command::cargo_bin("wrappex").expect("binary exists");
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("profile"))
        .stdout(predicate::str::contains("run"));
}

#[test]
fn profile_list_handles_missing_store() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut cmd = Command::cargo_bin("wrappex").expect("binary exists");
    with_home(&mut cmd, temp.path())
        .args(["profile", "list"])
        .assert()
        .success();
}

#[test]
fn run_invokes_codex_with_profile_overrides() {
    let temp = tempfile::tempdir().expect("tempdir");
    let wrappex_dir = temp.path().join(".wrappex");
    fs::create_dir_all(&wrappex_dir).expect("create wrappex dir");
    fs::write(
        wrappex_dir.join("profiles.toml"),
        r#"
version = 1

[[profiles]]
id = "unsloth-local"
name = "Unsloth Studio"
base_url = "http://127.0.0.1:9/v1"
wire_api = "responses"
requires_openai_auth = false
supports_websockets = false
default_model = "qwen"
request_max_retries = 0
stream_max_retries = 0
"#,
    )
    .expect("write profiles");

    let argv_path = temp.path().join("argv.txt");
    let codex_bin = fake_codex(temp.path(), &argv_path);

    let mut cmd = Command::cargo_bin("wrappex").expect("binary exists");
    with_home(&mut cmd, temp.path())
        .arg("--codex-bin")
        .arg(&codex_bin)
        .args(["run", "unsloth-local", "--", "--sandbox", "workspace-write"])
        .assert()
        .success();

    let argv = fs::read_to_string(argv_path).expect("read captured argv");
    assert!(argv.contains("--model"));
    assert!(argv.contains("qwen"));
    assert!(argv.contains("model_provider=unsloth-local"));
    assert!(argv.contains("model_providers.unsloth-local.base_url="));
    assert!(argv.contains("http://127.0.0.1:9/v1"));
    assert!(argv.contains("--sandbox"));
    assert!(argv.contains("workspace-write"));
}

fn with_home<'a>(cmd: &'a mut Command, home: &Path) -> &'a mut Command {
    cmd.env("USERPROFILE", home).env("HOME", home)
}

#[cfg(windows)]
fn fake_codex(dir: &Path, argv_path: &Path) -> std::path::PathBuf {
    let path = dir.join("codex.cmd");
    fs::write(
        &path,
        format!(
            "@echo off\r\necho %* > \"{}\"\r\nexit /b 0\r\n",
            argv_path.display()
        ),
    )
    .expect("write fake codex");
    path
}

#[cfg(not(windows))]
fn fake_codex(dir: &Path, argv_path: &Path) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = dir.join("codex");
    fs::write(
        &path,
        format!(
            "#!/bin/sh\nprintf '%s\n' \"$@\" > '{}'\n",
            argv_path.display()
        ),
    )
    .expect("write fake codex");
    let mut permissions = fs::metadata(&path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&path, permissions).expect("chmod");
    path
}
