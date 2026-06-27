use crate::profile::Profile;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LaunchError {
    #[error("could not find original codex binary; searched: {0}")]
    CodexNotFound(String),
    #[error("failed to launch codex at {path}: {source}")]
    Spawn {
        path: String,
        source: std::io::Error,
    },
}

pub fn build_codex_args(
    profile: &Profile,
    model: Option<&str>,
    passthrough: &[String],
) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(model) = model {
        args.push("--model".to_string());
        args.push(model.to_string());
    }
    push_override(&mut args, format!("model_provider={}", profile.id));
    push_override(
        &mut args,
        format!(
            "model_providers.{}.name={}",
            profile.id,
            toml_string(&profile.name)
        ),
    );
    push_override(
        &mut args,
        format!(
            "model_providers.{}.base_url={}",
            profile.id,
            toml_string(&profile.base_url)
        ),
    );
    push_override(
        &mut args,
        format!(
            "model_providers.{}.wire_api={}",
            profile.id,
            toml_string(&profile.wire_api)
        ),
    );
    push_override(
        &mut args,
        format!(
            "model_providers.{}.requires_openai_auth={}",
            profile.id, profile.requires_openai_auth
        ),
    );
    push_override(
        &mut args,
        format!(
            "model_providers.{}.supports_websockets={}",
            profile.id, profile.supports_websockets
        ),
    );
    push_override(
        &mut args,
        format!(
            "model_providers.{}.request_max_retries={}",
            profile.id, profile.request_max_retries
        ),
    );
    push_override(
        &mut args,
        format!(
            "model_providers.{}.stream_max_retries={}",
            profile.id, profile.stream_max_retries
        ),
    );
    if let Some(env_key) = profile.env_key.as_deref().filter(|value| !value.is_empty()) {
        push_override(
            &mut args,
            format!(
                "model_providers.{}.env_key={}",
                profile.id,
                toml_string(env_key)
            ),
        );
    }
    if let Some(path) = profile.model_catalog_json.as_deref() {
        push_override(
            &mut args,
            format!(
                "model_catalog_json={}",
                toml_string(&path.display().to_string())
            ),
        );
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
    resolve_codex_bin(explicit, std::env::current_exe().ok(), |name| {
        which::which(name).ok()
    })
}

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
            model_catalog_json: Some(PathBuf::from(
                "C:/Users/Aristo/.wrappex/model-catalogs/unsloth-local.json",
            )),
            env_key: Some("UNSLOTH_API_KEY".to_string()),
            request_max_retries: 0,
            stream_max_retries: 0,
        }
    }

    #[test]
    fn builds_codex_args_with_provider_overrides() {
        let args = build_codex_args(
            &profile(),
            Some("qwen"),
            &["--sandbox".into(), "workspace-write".into()],
        );
        assert_eq!(args[0], "--model");
        assert_eq!(args[1], "qwen");
        assert!(args.contains(&"model_provider=unsloth-local".to_string()));
        assert!(args.contains(
            &"model_providers.unsloth-local.base_url=\"http://localhost:8001/v1\"".to_string()
        ));
        assert!(
            args.contains(&"model_providers.unsloth-local.env_key=\"UNSLOTH_API_KEY\"".to_string())
        );
        assert!(args.contains(
            &"model_catalog_json=\"C:/Users/Aristo/.wrappex/model-catalogs/unsloth-local.json\""
                .to_string()
        ));
        assert!(args.ends_with(&["--sandbox".to_string(), "workspace-write".to_string()]));
    }

    #[test]
    fn omits_model_arg_when_model_is_not_selected() {
        let args = build_codex_args(&profile(), None, &[]);

        assert!(!args.contains(&"--model".to_string()));
        assert!(args.contains(&"model_provider=unsloth-local".to_string()));
    }

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
}
