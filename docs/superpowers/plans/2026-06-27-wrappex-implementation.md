# Wrappex Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust `wrappex` CLI wrapper that stores local Codex provider profiles in `~/.wrappex`, discovers OpenAI-compatible models, and launches the original `codex` binary with runtime provider overrides.

**Architecture:** Create a standalone Cargo binary crate with small modules for profile storage, model discovery, Codex argument construction, binary resolution, process launch, and interactive prompts. Keep prompt code thin and put all important behavior behind testable pure functions or injected dependencies.

**Tech Stack:** Rust 2021, `clap`, `serde`, `toml`, `serde_json`, `reqwest` blocking client, `inquire`, `dirs`, `which`, `anyhow`, `thiserror`, `tempfile`, `assert_cmd`, `predicates`.

---

## File Structure

- Create `Cargo.toml`: package metadata, runtime dependencies, dev dependencies.
- Create `src/main.rs`: process entrypoint, error printing, exit-code handoff.
- Create `src/cli.rs`: `clap` command definitions and pass-through argument parsing.
- Create `src/profile.rs`: profile structs, TOML load/save, id validation, name-to-id derivation, duplicate checks.
- Create `src/models.rs`: model response parser and `/models` HTTP fetch.
- Create `src/launch.rs`: Codex binary resolution, Codex argument construction, child process execution.
- Create `src/ui.rs`: interactive menus and profile creation wizard.
- Create `src/app.rs`: command orchestration between CLI, UI, storage, discovery, and launch.
- Create `tests/cli_smoke.rs`: binary-level smoke tests that do not launch real Codex.

## Task 1: Scaffold Cargo Project

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/lib.rs`

- [ ] **Step 1: Write the first failing smoke test**

Create `tests/cli_smoke.rs`:

```rust
use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn help_mentions_profile_commands() {
    let mut cmd = Command::cargo_bin("wrappex").expect("binary exists");
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("profile"))
        .stdout(predicate::str::contains("run"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test cli_smoke help_mentions_profile_commands`

Expected: FAIL because the Cargo package and binary do not exist yet.

- [ ] **Step 3: Add minimal crate scaffolding**

Create `Cargo.toml`:

```toml
[package]
name = "wrappex"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
dirs = "5"
inquire = "0.7"
reqwest = { version = "0.12", default-features = false, features = ["blocking", "json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
toml = "0.8"
which = "6"

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

Create `src/lib.rs`:

```rust
pub mod app;
pub mod cli;
pub mod launch;
pub mod models;
pub mod profile;
pub mod ui;
```

Create `src/main.rs`:

```rust
fn main() {
    if let Err(error) = wrappex::app::run_from_env() {
        eprintln!("wrappex: {error:#}");
        std::process::exit(1);
    }
}
```

- [ ] **Step 4: Add minimal CLI module**

Create `src/cli.rs`:

```rust
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "wrappex", version, about = "Launch Codex with local model profiles")]
pub struct Cli {
    #[arg(long = "codex-bin", global = true)]
    pub codex_bin: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Run(RunArgs),
    Profile(ProfileArgs),
}

#[derive(Debug, Args)]
pub struct ProfileArgs {
    #[command(subcommand)]
    pub command: ProfileCommand,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    pub profile: String,

    #[arg(last = true)]
    pub codex_args: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    Create,
    List,
    Remove { profile: String },
}
```

Create minimal modules so compilation reaches CLI parsing:

```rust
// src/app.rs
use anyhow::Result;
use clap::Parser;

pub fn run_from_env() -> Result<()> {
    let _cli = crate::cli::Cli::parse();
    Ok(())
}
```

Create empty files: `src/launch.rs`, `src/models.rs`, `src/profile.rs`, `src/ui.rs`.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --test cli_smoke help_mentions_profile_commands`

Expected: PASS.

## Task 2: Profile Storage Core

**Files:**
- Modify: `src/profile.rs`
- Test: `src/profile.rs`

- [ ] **Step 1: Write failing tests for profile id and TOML round trip**

Add to `src/profile.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_valid_profile_id_from_name() {
        assert_eq!(derive_profile_id("Unsloth Studio Local"), "unsloth-studio-local");
        assert_eq!(derive_profile_id("Qwen3_Coder 30B"), "qwen3_coder-30b");
    }

    #[test]
    fn rejects_invalid_profile_ids() {
        assert!(validate_profile_id("unsloth-local").is_ok());
        assert!(validate_profile_id("1bad").is_err());
        assert!(validate_profile_id("Bad").is_err());
        assert!(validate_profile_id("bad.profile").is_err());
    }

    #[test]
    fn loads_profiles_from_toml() {
        let raw = r#"
version = 1

[[profiles]]
id = "unsloth-local"
name = "Unsloth Studio"
base_url = "http://localhost:8001/v1"
wire_api = "responses"
requires_openai_auth = false
supports_websockets = false
default_model = "qwen"
request_max_retries = 0
stream_max_retries = 0
"#;

        let store: ProfileStore = toml::from_str(raw).expect("profile toml parses");
        assert_eq!(store.version, 1);
        assert_eq!(store.profiles[0].id, "unsloth-local");
        assert_eq!(store.profiles[0].default_model.as_deref(), Some("qwen"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test profile::tests`

Expected: FAIL because profile types and functions are undefined.

- [ ] **Step 3: Implement profile types and validation**

Add to `src/profile.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error("invalid profile id '{0}': use lowercase letters, digits, '_' or '-', starting with a letter")]
    InvalidId(String),
    #[error("duplicate profile id '{0}'")]
    DuplicateId(String),
    #[error("failed to read {path}: {source}")]
    Read { path: String, source: std::io::Error },
    #[error("failed to write {path}: {source}")]
    Write { path: String, source: std::io::Error },
    #[error("failed to parse {path}: {source}")]
    Parse { path: String, source: toml::de::Error },
    #[error("failed to serialize profiles: {0}")]
    Serialize(toml::ser::Error),
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProfileStore {
    pub version: u32,
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

impl Default for ProfileStore {
    fn default() -> Self {
        Self { version: 1, profiles: Vec::new() }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub base_url: String,
    #[serde(default = "default_wire_api")]
    pub wire_api: String,
    #[serde(default)]
    pub requires_openai_auth: bool,
    #[serde(default)]
    pub supports_websockets: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env_key: Option<String>,
    #[serde(default)]
    pub request_max_retries: u64,
    #[serde(default)]
    pub stream_max_retries: u64,
}

fn default_wire_api() -> String {
    "responses".to_string()
}

pub fn derive_profile_id(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in name.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
            last_dash = false;
        } else if !last_dash && !out.is_empty() {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.chars().next().is_some_and(|ch| ch.is_ascii_alphabetic()) {
        out
    } else {
        format!("profile-{out}")
    }
}

pub fn validate_profile_id(id: &str) -> Result<(), ProfileError> {
    let mut chars = id.chars();
    match chars.next() {
        Some(ch) if ch.is_ascii_lowercase() => {}
        _ => return Err(ProfileError::InvalidId(id.to_string())),
    }
    if chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-') {
        Ok(())
    } else {
        Err(ProfileError::InvalidId(id.to_string()))
    }
}

pub fn validate_store(store: &ProfileStore) -> Result<(), ProfileError> {
    let mut seen = HashSet::new();
    for profile in &store.profiles {
        validate_profile_id(&profile.id)?;
        if !seen.insert(profile.id.clone()) {
            return Err(ProfileError::DuplicateId(profile.id.clone()));
        }
    }
    Ok(())
}

pub fn load_profiles(path: &Path) -> Result<ProfileStore, ProfileError> {
    if !path.exists() {
        return Ok(ProfileStore::default());
    }
    let raw = fs::read_to_string(path).map_err(|source| ProfileError::Read {
        path: path.display().to_string(),
        source,
    })?;
    let store: ProfileStore = toml::from_str(&raw).map_err(|source| ProfileError::Parse {
        path: path.display().to_string(),
        source,
    })?;
    validate_store(&store)?;
    Ok(store)
}

pub fn save_profiles(path: &Path, store: &ProfileStore) -> Result<(), ProfileError> {
    validate_store(store)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ProfileError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let raw = toml::to_string_pretty(store).map_err(ProfileError::Serialize)?;
    fs::write(path, raw).map_err(|source| ProfileError::Write {
        path: path.display().to_string(),
        source,
    })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test profile::tests`

Expected: PASS.

## Task 3: Model Response Parsing and Discovery

**Files:**
- Modify: `src/models.rs`
- Test: `src/models.rs`

- [ ] **Step 1: Write failing parser tests**

Add to `src/models.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openai_compatible_model_ids() {
        let raw = r#"{"data":[{"id":"z-model"},{"id":"a-model"}]}"#;
        assert_eq!(parse_model_ids(raw).unwrap(), vec!["a-model", "z-model"]);
    }

    #[test]
    fn ignores_entries_without_string_ids() {
        let raw = r#"{"data":[{"id":"qwen"},{"id":42},{"object":"model"}]}"#;
        assert_eq!(parse_model_ids(raw).unwrap(), vec!["qwen"]);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test models::tests`

Expected: FAIL because `parse_model_ids` is undefined.

- [ ] **Step 3: Implement parser and HTTP discovery**

Add to `src/models.rs`:

```rust
use crate::profile::Profile;
use reqwest::blocking::Client;
use serde::Deserialize;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("model endpoint request failed: {0}")]
    Request(reqwest::Error),
    #[error("model endpoint returned invalid JSON: {0}")]
    Json(serde_json::Error),
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<ModelItem>,
}

#[derive(Debug, Deserialize)]
struct ModelItem {
    id: Option<String>,
}

pub fn parse_model_ids(raw: &str) -> Result<Vec<String>, ModelError> {
    let response: ModelsResponse = serde_json::from_str(raw).map_err(ModelError::Json)?;
    let mut ids: Vec<String> = response.data.into_iter().filter_map(|item| item.id).collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

pub fn models_url(base_url: &str) -> String {
    format!("{}/models", base_url.trim_end_matches('/'))
}

pub fn fetch_model_ids(profile: &Profile) -> Result<Vec<String>, ModelError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(ModelError::Request)?;
    let mut request = client.get(models_url(&profile.base_url));
    if let Some(env_key) = profile.env_key.as_deref().filter(|value| !value.is_empty()) {
        if let Ok(token) = std::env::var(env_key) {
            request = request.bearer_auth(token);
        }
    }
    let body = request
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(ModelError::Request)?
        .text()
        .map_err(ModelError::Request)?;
    parse_model_ids(&body)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test models::tests`

Expected: PASS.

## Task 4: Codex Argument Construction

**Files:**
- Modify: `src/launch.rs`
- Test: `src/launch.rs`

- [ ] **Step 1: Write failing argument construction tests**

Add to `src/launch.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::Profile;

    fn profile() -> Profile {
        Profile {
            id: "unsloth-local".to_string(),
            name: "Unsloth Studio".to_string(),
            base_url: "http://localhost:8001/v1".to_string(),
            wire_api: "responses".to_string(),
            requires_openai_auth: false,
            supports_websockets: false,
            default_model: Some("qwen".to_string()),
            env_key: Some("UNSLOTH_API_KEY".to_string()),
            request_max_retries: 0,
            stream_max_retries: 0,
        }
    }

    #[test]
    fn builds_codex_args_with_provider_overrides() {
        let args = build_codex_args(&profile(), "qwen", &["--sandbox".into(), "workspace-write".into()]);
        assert_eq!(args[0], "--model");
        assert_eq!(args[1], "qwen");
        assert!(args.contains(&"model_provider=unsloth-local".to_string()));
        assert!(args.contains(&"model_providers.unsloth-local.base_url=\"http://localhost:8001/v1\"".to_string()));
        assert!(args.contains(&"model_providers.unsloth-local.env_key=\"UNSLOTH_API_KEY\"".to_string()));
        assert!(args.ends_with(&["--sandbox".to_string(), "workspace-write".to_string()]));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test launch::tests::builds_codex_args_with_provider_overrides`

Expected: FAIL because `build_codex_args` is undefined.

- [ ] **Step 3: Implement TOML-safe argument builder**

Add to `src/launch.rs`:

```rust
use crate::profile::Profile;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LaunchError {
    #[error("could not find original codex binary; searched: {0}")]
    CodexNotFound(String),
    #[error("failed to launch codex at {path}: {source}")]
    Spawn { path: String, source: std::io::Error },
}

pub fn build_codex_args(profile: &Profile, model: &str, passthrough: &[String]) -> Vec<String> {
    let mut args = vec!["--model".to_string(), model.to_string()];
    push_override(&mut args, format!("model_provider={}", profile.id));
    push_override(&mut args, format!("model_providers.{}.name={}", profile.id, toml_string(&profile.name)));
    push_override(&mut args, format!("model_providers.{}.base_url={}", profile.id, toml_string(&profile.base_url)));
    push_override(&mut args, format!("model_providers.{}.wire_api={}", profile.id, toml_string(&profile.wire_api)));
    push_override(&mut args, format!("model_providers.{}.requires_openai_auth={}", profile.id, profile.requires_openai_auth));
    push_override(&mut args, format!("model_providers.{}.supports_websockets={}", profile.id, profile.supports_websockets));
    push_override(&mut args, format!("model_providers.{}.request_max_retries={}", profile.id, profile.request_max_retries));
    push_override(&mut args, format!("model_providers.{}.stream_max_retries={}", profile.id, profile.stream_max_retries));
    if let Some(env_key) = profile.env_key.as_deref().filter(|value| !value.is_empty()) {
        push_override(&mut args, format!("model_providers.{}.env_key={}", profile.id, toml_string(env_key)));
    }
    args.extend_from_slice(passthrough);
    args
}

fn push_override(args: &mut Vec<String>, value: String) {
    args.push("-c".to_string());
    args.push(value);
}

fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

pub fn run_codex(codex_bin: &Path, args: &[String]) -> Result<i32, LaunchError> {
    let status = Command::new(codex_bin)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|source| LaunchError::Spawn {
            path: codex_bin.display().to_string(),
            source,
        })?;
    Ok(status.code().unwrap_or(1))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test launch::tests::builds_codex_args_with_provider_overrides`

Expected: PASS.

## Task 5: Codex Binary Resolution

**Files:**
- Modify: `src/launch.rs`
- Test: `src/launch.rs`

- [ ] **Step 1: Write failing resolver tests**

Add to `src/launch.rs` test module:

```rust
#[test]
fn explicit_codex_bin_wins() {
    let explicit = PathBuf::from("C:/tools/codex.exe");
    let found = resolve_codex_bin(Some(explicit.clone()), None, |_| None).unwrap();
    assert_eq!(found, explicit);
}

#[test]
fn finds_sibling_codex_exe() {
    let temp = tempfile::tempdir().unwrap();
    let wrappex = temp.path().join("wrappex.exe");
    let codex = temp.path().join("codex.exe");
    std::fs::write(&wrappex, "").unwrap();
    std::fs::write(&codex, "").unwrap();

    let found = resolve_codex_bin(None, Some(wrappex), |_| None).unwrap();
    assert_eq!(found, codex);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test launch::tests`

Expected: FAIL because `resolve_codex_bin` is undefined.

- [ ] **Step 3: Implement resolver with injected PATH lookup**

Add to `src/launch.rs`:

```rust
pub fn resolve_codex_bin<F>(
    explicit: Option<PathBuf>,
    current_exe: Option<PathBuf>,
    path_lookup: F,
) -> Result<PathBuf, LaunchError>
where
    F: Fn(&str) -> Option<PathBuf>,
{
    let mut searched = Vec::new();
    if let Some(path) = explicit {
        return Ok(path);
    }
    if let Ok(path) = std::env::var("WRAPPEX_CODEX_BIN") {
        return Ok(PathBuf::from(path));
    }
    if let Some(current_exe) = current_exe {
        if let Some(parent) = current_exe.parent() {
            for name in ["codex.exe", "codex"] {
                let candidate = parent.join(name);
                searched.push(candidate.display().to_string());
                if candidate.exists() {
                    return Ok(candidate);
                }
            }
        }
    }
    for name in ["codex.exe", "codex"] {
        if let Some(found) = path_lookup(name) {
            return Ok(found);
        }
        searched.push(format!("PATH:{name}"));
    }
    Err(LaunchError::CodexNotFound(searched.join(", ")))
}

pub fn resolve_codex_bin_from_env(explicit: Option<PathBuf>) -> Result<PathBuf, LaunchError> {
    resolve_codex_bin(explicit, std::env::current_exe().ok(), |name| which::which(name).ok())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test launch::tests`

Expected: PASS.

## Task 6: Interactive UI and Command Orchestration

**Files:**
- Modify: `src/ui.rs`
- Modify: `src/app.rs`
- Test: `src/app.rs`

- [ ] **Step 1: Write failing app-level tests for profile lookup**

Add to `src/app.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::{Profile, ProfileStore};

    #[test]
    fn finds_profile_by_id_or_name_case_insensitive() {
        let store = ProfileStore {
            version: 1,
            profiles: vec![Profile {
                id: "unsloth-local".into(),
                name: "Unsloth Studio".into(),
                base_url: "http://localhost:8001/v1".into(),
                wire_api: "responses".into(),
                requires_openai_auth: false,
                supports_websockets: false,
                default_model: Some("qwen".into()),
                env_key: None,
                request_max_retries: 0,
                stream_max_retries: 0,
            }],
        };

        assert_eq!(find_profile(&store, "unsloth-local").unwrap().name, "Unsloth Studio");
        assert_eq!(find_profile(&store, "unsloth studio").unwrap().id, "unsloth-local");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test app::tests::finds_profile_by_id_or_name_case_insensitive`

Expected: FAIL because `find_profile` is undefined.

- [ ] **Step 3: Implement app lookup and config path**

Add to `src/app.rs`:

```rust
use crate::cli::{Cli, Command as CliCommand, ProfileCommand};
use crate::profile::{load_profiles, save_profiles, Profile, ProfileStore};
use anyhow::{anyhow, Result};
use clap::Parser;
use std::path::PathBuf;

pub fn default_profiles_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("could not determine home directory"))?;
    Ok(home.join(".wrappex").join("profiles.toml"))
}

pub fn find_profile<'a>(store: &'a ProfileStore, query: &str) -> Option<&'a Profile> {
    let normalized = query.to_ascii_lowercase();
    store.profiles.iter().find(|profile| {
        profile.id == query || profile.name.to_ascii_lowercase() == normalized
    })
}

pub fn run_from_env() -> Result<()> {
    let cli = Cli::parse();
    let profiles_path = default_profiles_path()?;
    let mut store = load_profiles(&profiles_path)?;

    match cli.command {
        Some(CliCommand::Profile(args)) => match args.command {
            ProfileCommand::List => {
                crate::ui::print_profiles(&store);
                Ok(())
            }
            ProfileCommand::Create => {
                let profile = crate::ui::prompt_new_profile()?;
                store.profiles.push(profile);
                save_profiles(&profiles_path, &store)?;
                Ok(())
            }
            ProfileCommand::Remove { profile } => {
                crate::ui::remove_profile_interactive(&mut store, &profile)?;
                save_profiles(&profiles_path, &store)?;
                Ok(())
            }
        },
        Some(CliCommand::Run(args)) => {
            let profile = find_profile(&store, &args.profile)
                .ok_or_else(|| anyhow!("profile '{}' not found", args.profile))?
                .clone();
            run_profile(cli.codex_bin, profile, args.codex_args)
        }
        None => {
            let profile = crate::ui::select_or_create_profile(&mut store)?;
            save_profiles(&profiles_path, &store)?;
            run_profile(cli.codex_bin, profile, Vec::new())
        }
    }
}

fn run_profile(codex_bin: Option<PathBuf>, profile: Profile, passthrough: Vec<String>) -> Result<()> {
    crate::ui::warn_missing_env_key(&profile)?;
    let model = crate::ui::select_model(&profile)?;
    let codex_bin = crate::launch::resolve_codex_bin_from_env(codex_bin)?;
    let args = crate::launch::build_codex_args(&profile, &model, &passthrough);
    let code = crate::launch::run_codex(&codex_bin, &args)?;
    std::process::exit(code);
}
```

- [ ] **Step 4: Implement interactive UI functions**

Add to `src/ui.rs`:

```rust
use crate::models::fetch_model_ids;
use crate::profile::{derive_profile_id, validate_profile_id, Profile, ProfileStore};
use anyhow::{anyhow, Result};
use inquire::{Confirm, Select, Text};

const CREATE_PROFILE: &str = "Create profile";

pub fn print_profiles(store: &ProfileStore) {
    for profile in &store.profiles {
        let model = profile.default_model.as_deref().unwrap_or("<no default model>");
        println!("{} ({}) - {} - {}", profile.name, profile.id, profile.base_url, model);
    }
}

pub fn select_or_create_profile(store: &mut ProfileStore) -> Result<Profile> {
    let mut choices: Vec<String> = store
        .profiles
        .iter()
        .map(|profile| format!("{} ({})", profile.name, profile.id))
        .collect();
    choices.push(CREATE_PROFILE.to_string());
    let selected = Select::new("Choose wrappex profile", choices).prompt()?;
    if selected == CREATE_PROFILE {
        let profile = prompt_new_profile()?;
        store.profiles.push(profile.clone());
        Ok(profile)
    } else {
        let id_start = selected.rfind('(').ok_or_else(|| anyhow!("invalid selection"))? + 1;
        let id_end = selected.rfind(')').ok_or_else(|| anyhow!("invalid selection"))?;
        let id = &selected[id_start..id_end];
        store.profiles
            .iter()
            .find(|profile| profile.id == id)
            .cloned()
            .ok_or_else(|| anyhow!("selected profile not found"))
    }
}

pub fn prompt_new_profile() -> Result<Profile> {
    let name = Text::new("Profile name")
        .with_default("Unsloth Studio")
        .prompt()?;
    let default_id = derive_profile_id(&name);
    let id = Text::new("Provider id")
        .with_default(&default_id)
        .prompt()?;
    validate_profile_id(&id)?;
    let base_url = Text::new("Base URL")
        .with_default("http://localhost:8001/v1")
        .prompt()?;
    let wire_api = Text::new("Wire API")
        .with_default("responses")
        .prompt()?;
    let requires_openai_auth = Confirm::new("Requires OpenAI auth?")
        .with_default(false)
        .prompt()?;
    let env_key = if Confirm::new("Use API key from environment variable?")
        .with_default(false)
        .prompt()?
    {
        Some(Text::new("Environment variable").prompt()?)
    } else {
        None
    };
    let mut profile = Profile {
        id,
        name,
        base_url,
        wire_api,
        requires_openai_auth,
        supports_websockets: false,
        default_model: None,
        env_key,
        request_max_retries: 0,
        stream_max_retries: 0,
    };
    profile.default_model = prompt_model_for_profile(&profile)?;
    Ok(profile)
}

pub fn select_model(profile: &Profile) -> Result<String> {
    if let Some(model) = prompt_model_for_profile(profile)? {
        return Ok(model);
    }
    if let Some(model) = profile.default_model.clone() {
        return Ok(model);
    }
    Text::new("Model").prompt().map_err(Into::into)
}

fn prompt_model_for_profile(profile: &Profile) -> Result<Option<String>> {
    match fetch_model_ids(profile) {
        Ok(models) if !models.is_empty() => {
            let initial = profile
                .default_model
                .as_ref()
                .and_then(|default| models.iter().position(|model| model == default))
                .unwrap_or(0);
            Select::new("Choose model", models)
                .with_starting_cursor(initial)
                .prompt()
                .map(Some)
                .map_err(Into::into)
        }
        Ok(_) => Ok(None),
        Err(error) => {
            eprintln!("wrappex: could not fetch models: {error}");
            Ok(None)
        }
    }
}

pub fn warn_missing_env_key(profile: &Profile) -> Result<()> {
    if let Some(env_key) = profile.env_key.as_deref().filter(|value| !value.is_empty()) {
        if std::env::var_os(env_key).is_none() {
            let proceed = Confirm::new(&format!("{env_key} is not set. Continue?"))
                .with_default(false)
                .prompt()?;
            if !proceed {
                anyhow::bail!("launch cancelled");
            }
        }
    }
    Ok(())
}

pub fn remove_profile_interactive(store: &mut ProfileStore, query: &str) -> Result<()> {
    let index = store
        .profiles
        .iter()
        .position(|profile| profile.id == query || profile.name.eq_ignore_ascii_case(query))
        .ok_or_else(|| anyhow!("profile '{query}' not found"))?;
    let profile = &store.profiles[index];
    if Confirm::new(&format!("Remove profile {}?", profile.name))
        .with_default(false)
        .prompt()?
    {
        store.profiles.remove(index);
    }
    Ok(())
}
```

- [ ] **Step 5: Run app and UI-related tests**

Run: `cargo test app::tests`

Expected: PASS.

## Task 7: CLI Smoke Tests and Formatting

**Files:**
- Modify: `tests/cli_smoke.rs`
- Modify: `src/*`

- [ ] **Step 1: Add smoke tests for `profile list` with isolated HOME**

Append to `tests/cli_smoke.rs`:

```rust
#[test]
fn profile_list_handles_missing_store() {
    let temp = tempfile::tempdir().expect("tempdir");
    let mut cmd = Command::cargo_bin("wrappex").expect("binary exists");
    cmd.env("USERPROFILE", temp.path())
        .env("HOME", temp.path())
        .args(["profile", "list"])
        .assert()
        .success();
}
```

- [ ] **Step 2: Run smoke tests**

Run: `cargo test --test cli_smoke`

Expected: PASS. If `dirs::home_dir()` ignores test env on the host, refactor `app` to accept an injected profiles path and keep `run_from_env` as the thin real-env wrapper.

- [ ] **Step 3: Format code**

Run: `cargo fmt`

Expected: no output and exit code `0`.

- [ ] **Step 4: Run full test suite**

Run: `cargo test`

Expected: PASS.

## Task 8: Build and Manual Verification

**Files:**
- Create: `README.md`

- [ ] **Step 1: Add concise usage documentation**

Create `README.md`:

````markdown
# wrappex

`wrappex` launches the original Codex CLI with local OpenAI-compatible model provider profiles stored in `~/.wrappex/profiles.toml`.

## Build

```powershell
cargo build --release
```

## Usage

```powershell
wrappex
wrappex profile create
wrappex profile list
wrappex run unsloth-local -- --sandbox workspace-write
```

Set `WRAPPEX_CODEX_BIN` or pass `--codex-bin` if `codex` is not next to `wrappex` and not on `PATH`.
````

- [ ] **Step 2: Build release binary**

Run: `cargo build --release`

Expected: PASS and binary at `target/release/wrappex.exe` on Windows or `target/release/wrappex` on Unix.

- [ ] **Step 3: Verify help output**

Run: `target\release\wrappex.exe --help`

Expected: help includes `run`, `profile`, and `--codex-bin`.

- [ ] **Step 4: Verify profile list on empty store**

Run: `target\release\wrappex.exe profile list`

Expected: exits `0`. It may print nothing when no profiles exist.

- [ ] **Step 5: Verify command construction without launching real Codex**

Create a temporary fake Codex command that records argv, point `--codex-bin` to it, and run `wrappex run` against a temporary profile store. On Windows, use a `codex.cmd` file that writes `%*` to an output file and exits `0`; on Unix, use a shell script that writes `"$@"` to an output file and exits `0`. Expected: the recorded args contain `--model`, `-c model_provider=<id>`, provider `base_url`, and pass-through args.

- [ ] **Step 6: Final verification**

Run:

```powershell
cargo fmt --check
cargo test
cargo build --release
```

Expected: all commands exit `0`.

## Self-Review Checklist

- [ ] Spec coverage: separate `~/.wrappex` storage, profile wizard, `/models` discovery, `codex` launch overrides, binary resolution, pass-through args, and non-goals are covered.
- [ ] Completeness scan: all implementation steps are concrete and executable.
- [ ] Type consistency: `Profile`, `ProfileStore`, `build_codex_args`, `resolve_codex_bin`, `fetch_model_ids`, and `run_from_env` names are consistent across tasks.
- [ ] TDD compliance: every production module has a failing test before its main behavior is implemented.
