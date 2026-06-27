use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProfileError {
    #[error(
        "invalid profile id '{0}': use lowercase letters, digits, '_' or '-', starting with a letter"
    )]
    InvalidId(String),
    #[error("duplicate profile id '{0}'")]
    DuplicateId(String),
    #[error("failed to read {path}: {source}")]
    Read {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to write {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {source}")]
    Parse {
        path: String,
        source: toml::de::Error,
    },
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
        Self {
            version: 1,
            profiles: Vec::new(),
        }
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
    pub model_catalog_json: Option<PathBuf>,
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
    if out
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_alphabetic())
    {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_valid_profile_id_from_name() {
        assert_eq!(
            derive_profile_id("Unsloth Studio Local"),
            "unsloth-studio-local"
        );
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
model_catalog_json = "C:\\Users\\Aristo\\.wrappex\\model-catalogs\\unsloth-local.json"
request_max_retries = 0
stream_max_retries = 0
"#;

        let store: ProfileStore = toml::from_str(raw).expect("profile toml parses");
        assert_eq!(store.version, 1);
        assert_eq!(store.profiles[0].id, "unsloth-local");
        assert_eq!(store.profiles[0].default_model.as_deref(), Some("qwen"));
        assert_eq!(
            store.profiles[0]
                .model_catalog_json
                .as_deref()
                .map(Path::display)
                .map(|path| path.to_string())
                .as_deref(),
            Some("C:\\Users\\Aristo\\.wrappex\\model-catalogs\\unsloth-local.json")
        );
    }
}
