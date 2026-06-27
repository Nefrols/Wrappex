use crate::profile::Profile;
use reqwest::blocking::Client;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

const DEFAULT_BASE_INSTRUCTIONS: &str =
    "You are Codex, a coding agent. Follow the user's instructions and work carefully in the local workspace.";

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("model endpoint request failed: {0}")]
    Request(reqwest::Error),
    #[error("model endpoint returned invalid JSON: {0}")]
    Json(serde_json::Error),
    #[error("failed to serialize model catalog: {0}")]
    Serialize(serde_json::Error),
    #[error("failed to write model catalog {path}: {source}")]
    Write {
        path: String,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ModelCatalog {
    pub models: Vec<Value>,
}

impl ModelCatalog {
    pub fn ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .models
            .iter()
            .filter_map(|model| model.get("slug").and_then(Value::as_str))
            .map(str::to_string)
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    pub fn first_id(&self) -> Option<String> {
        self.models
            .iter()
            .find_map(|model| model.get("slug").and_then(Value::as_str))
            .map(str::to_string)
    }
}

pub fn parse_model_ids(raw: &str) -> Result<Vec<String>, ModelError> {
    parse_model_catalog(raw).map(|catalog| catalog.ids())
}

pub fn parse_model_catalog(raw: &str) -> Result<ModelCatalog, ModelError> {
    let value: Value = serde_json::from_str(raw).map_err(ModelError::Json)?;
    let model_values = value
        .get("models")
        .and_then(Value::as_array)
        .or_else(|| value.get("data").and_then(Value::as_array))
        .cloned()
        .unwrap_or_default();

    let mut seen = HashSet::new();
    let models: Vec<Value> = model_values
        .iter()
        .filter_map(normalize_model_item)
        .filter(|model| seen.insert(model_slug(model).to_string()))
        .collect();
    Ok(ModelCatalog { models })
}

pub fn models_url(base_url: &str) -> String {
    format!("{}/models", base_url.trim_end_matches('/'))
}

pub fn model_catalog_path(wrappex_dir: &Path, profile_id: &str) -> PathBuf {
    wrappex_dir
        .join("model-catalogs")
        .join(format!("{profile_id}.json"))
}

pub fn fetch_model_catalog(profile: &Profile) -> Result<ModelCatalog, ModelError> {
    let body = fetch_models_body(profile)?;
    parse_model_catalog(&body)
}

pub fn fetch_model_ids(profile: &Profile) -> Result<Vec<String>, ModelError> {
    fetch_model_catalog(profile).map(|catalog| catalog.ids())
}

pub fn refresh_profile_model_catalog(
    profile: &mut Profile,
    wrappex_dir: &Path,
) -> Result<(), ModelError> {
    let catalog = fetch_model_catalog(profile)?;
    apply_model_catalog_to_profile(profile, wrappex_dir, &catalog)
}

pub fn apply_model_catalog_to_profile(
    profile: &mut Profile,
    wrappex_dir: &Path,
    catalog: &ModelCatalog,
) -> Result<(), ModelError> {
    if catalog.is_empty() {
        return Ok(());
    }
    let path = model_catalog_path(wrappex_dir, &profile.id);
    write_model_catalog(&path, &catalog)?;
    profile.model_catalog_json = Some(path);
    if profile.default_model.is_none() {
        profile.default_model = catalog.first_id();
    }
    Ok(())
}

fn fetch_models_body(profile: &Profile) -> Result<String, ModelError> {
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
    request
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(ModelError::Request)?
        .text()
        .map_err(ModelError::Request)
}

fn write_model_catalog(path: &Path, catalog: &ModelCatalog) -> Result<(), ModelError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| ModelError::Write {
            path: parent.display().to_string(),
            source,
        })?;
    }
    let body = serde_json::to_string_pretty(catalog).map_err(ModelError::Serialize)?;
    fs::write(path, body).map_err(|source| ModelError::Write {
        path: path.display().to_string(),
        source,
    })
}

fn normalize_model_item(item: &Value) -> Option<Value> {
    let object = item.as_object()?;
    let slug = string_field(object, "slug").or_else(|| string_field(object, "id"))?;
    let display_name = string_field(object, "display_name")
        .or_else(|| string_field(object, "name"))
        .unwrap_or(slug);

    let mut model = default_codex_model(slug, display_name);
    copy_string(object, &mut model, "description");
    copy_string(object, &mut model, "default_reasoning_level");
    copy_string(object, &mut model, "shell_type");
    copy_string(object, &mut model, "visibility");
    copy_string(object, &mut model, "default_service_tier");
    copy_string(object, &mut model, "default_reasoning_summary");
    copy_string(object, &mut model, "default_verbosity");
    copy_string(object, &mut model, "apply_patch_tool_type");
    copy_string(object, &mut model, "web_search_tool_type");
    copy_string(object, &mut model, "comp_hash");
    copy_string(object, &mut model, "auto_review_model_override");
    copy_string(object, &mut model, "tool_mode");
    copy_string(object, &mut model, "multi_agent_version");

    copy_bool(object, &mut model, "supported_in_api");
    copy_bool(object, &mut model, "supports_reasoning_summaries");
    copy_bool(object, &mut model, "support_verbosity");
    copy_bool(object, &mut model, "supports_parallel_tool_calls");
    copy_bool(object, &mut model, "supports_image_detail_original");
    copy_bool(object, &mut model, "supports_search_tool");
    copy_bool(object, &mut model, "use_responses_lite");

    copy_i64(object, &mut model, "priority");
    copy_i64(object, &mut model, "context_window");
    copy_i64(object, &mut model, "max_context_window");
    copy_i64(object, &mut model, "auto_compact_token_limit");
    copy_i64(object, &mut model, "effective_context_window_percent");

    copy_array(object, &mut model, "supported_reasoning_levels");
    copy_array(object, &mut model, "additional_speed_tiers");
    copy_array(object, &mut model, "service_tiers");
    copy_array(object, &mut model, "experimental_supported_tools");
    copy_array(object, &mut model, "input_modalities");

    copy_object(object, &mut model, "availability_nux");
    copy_object(object, &mut model, "upgrade");
    copy_object(object, &mut model, "model_messages");
    copy_object(object, &mut model, "truncation_policy");

    if model
        .get("context_window")
        .and_then(Value::as_i64)
        .is_none()
    {
        if let Some(context_window) = context_window_from_metadata(object) {
            model.insert("context_window".to_string(), Value::from(context_window));
            model.insert(
                "max_context_window".to_string(),
                Value::from(context_window),
            );
        }
    }
    if model
        .get("max_context_window")
        .and_then(Value::as_i64)
        .is_none()
    {
        if let Some(context_window) = model.get("context_window").and_then(Value::as_i64) {
            model.insert(
                "max_context_window".to_string(),
                Value::from(context_window),
            );
        }
    }

    model.insert(
        "_wrappex_raw_model".to_string(),
        Value::Object(object.clone()),
    );
    Some(Value::Object(model))
}

fn default_codex_model(slug: &str, display_name: &str) -> Map<String, Value> {
    let mut model = Map::new();
    model.insert("slug".to_string(), Value::String(slug.to_string()));
    model.insert(
        "display_name".to_string(),
        Value::String(display_name.to_string()),
    );
    model.insert("description".to_string(), Value::Null);
    model.insert("default_reasoning_level".to_string(), Value::Null);
    model.insert(
        "supported_reasoning_levels".to_string(),
        Value::Array(vec![]),
    );
    model.insert(
        "shell_type".to_string(),
        Value::String("default".to_string()),
    );
    model.insert("visibility".to_string(), Value::String("list".to_string()));
    model.insert("supported_in_api".to_string(), Value::Bool(true));
    model.insert("priority".to_string(), Value::from(99));
    model.insert("additional_speed_tiers".to_string(), Value::Array(vec![]));
    model.insert("service_tiers".to_string(), Value::Array(vec![]));
    model.insert("default_service_tier".to_string(), Value::Null);
    model.insert("availability_nux".to_string(), Value::Null);
    model.insert("upgrade".to_string(), Value::Null);
    model.insert(
        "base_instructions".to_string(),
        Value::String(DEFAULT_BASE_INSTRUCTIONS.to_string()),
    );
    model.insert(
        "supports_reasoning_summaries".to_string(),
        Value::Bool(false),
    );
    model.insert(
        "default_reasoning_summary".to_string(),
        Value::String("auto".to_string()),
    );
    model.insert("support_verbosity".to_string(), Value::Bool(false));
    model.insert("default_verbosity".to_string(), Value::Null);
    model.insert("apply_patch_tool_type".to_string(), Value::Null);
    model.insert(
        "web_search_tool_type".to_string(),
        Value::String("text".to_string()),
    );
    model.insert(
        "truncation_policy".to_string(),
        serde_json::json!({"mode": "bytes", "limit": 10000}),
    );
    model.insert(
        "supports_parallel_tool_calls".to_string(),
        Value::Bool(false),
    );
    model.insert(
        "supports_image_detail_original".to_string(),
        Value::Bool(false),
    );
    model.insert("context_window".to_string(), Value::Null);
    model.insert("max_context_window".to_string(), Value::Null);
    model.insert("auto_compact_token_limit".to_string(), Value::Null);
    model.insert(
        "effective_context_window_percent".to_string(),
        Value::from(95),
    );
    model.insert(
        "experimental_supported_tools".to_string(),
        Value::Array(vec![]),
    );
    model.insert(
        "input_modalities".to_string(),
        Value::Array(vec![
            Value::String("text".into()),
            Value::String("image".into()),
        ]),
    );
    model.insert("supports_search_tool".to_string(), Value::Bool(false));
    model.insert("use_responses_lite".to_string(), Value::Bool(false));
    model.insert("auto_review_model_override".to_string(), Value::Null);
    model.insert("tool_mode".to_string(), Value::Null);
    model.insert("multi_agent_version".to_string(), Value::Null);
    model
}

fn model_slug(model: &Value) -> &str {
    model.get("slug").and_then(Value::as_str).unwrap_or("")
}

fn string_field<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get(key).and_then(Value::as_str)
}

fn copy_string(source: &Map<String, Value>, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key).and_then(Value::as_str) {
        target.insert(key.to_string(), Value::String(value.to_string()));
    }
}

fn copy_bool(source: &Map<String, Value>, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key).and_then(Value::as_bool) {
        target.insert(key.to_string(), Value::Bool(value));
    }
}

fn copy_i64(source: &Map<String, Value>, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key).and_then(Value::as_i64) {
        target.insert(key.to_string(), Value::from(value));
    }
}

fn copy_array(source: &Map<String, Value>, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key).and_then(Value::as_array) {
        target.insert(key.to_string(), Value::Array(value.clone()));
    }
}

fn copy_object(source: &Map<String, Value>, target: &mut Map<String, Value>, key: &str) {
    if let Some(value) = source.get(key).and_then(Value::as_object) {
        target.insert(key.to_string(), Value::Object(value.clone()));
    }
}

fn context_window_from_metadata(object: &Map<String, Value>) -> Option<i64> {
    for key in [
        "context_length",
        "max_context_length",
        "max_model_len",
        "n_ctx",
        "n_ctx_train",
        "ctx_size",
    ] {
        if let Some(value) = object.get(key).and_then(Value::as_i64) {
            return Some(value);
        }
    }
    object
        .get("metadata")
        .and_then(Value::as_object)
        .and_then(context_window_from_metadata)
}

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

    #[test]
    fn builds_codex_catalog_from_openai_compatible_metadata() {
        let raw = r#"{
            "data": [
                {
                    "id": "qwopus3.6-27b-coder-mtp",
                    "name": "Qwopus Coder",
                    "metadata": {
                        "context_length": 131072
                    },
                    "supports_parallel_tool_calls": true
                }
            ]
        }"#;

        let catalog = parse_model_catalog(raw).unwrap();
        let model = &catalog.models[0];
        assert_eq!(model["slug"], "qwopus3.6-27b-coder-mtp");
        assert_eq!(model["display_name"], "Qwopus Coder");
        assert_eq!(model["context_window"], 131072);
        assert_eq!(model["max_context_window"], 131072);
        assert_eq!(model["supports_parallel_tool_calls"], true);
        assert_eq!(model["_wrappex_raw_model"]["id"], "qwopus3.6-27b-coder-mtp");
    }

    #[test]
    fn preserves_api_order_for_default_model_selection() {
        let raw = r#"{"data":[{"id":"z-loaded-first"},{"id":"a-loaded-second"}]}"#;

        let catalog = parse_model_catalog(raw).unwrap();

        assert_eq!(catalog.models[0]["slug"], "z-loaded-first");
        assert_eq!(catalog.first_id().as_deref(), Some("z-loaded-first"));
        assert_eq!(
            parse_model_ids(raw).unwrap(),
            vec!["a-loaded-second", "z-loaded-first"]
        );
    }

    #[test]
    fn applies_catalog_path_and_first_model_when_profile_has_no_default() {
        let mut profile = Profile {
            id: "unsloth-local".to_string(),
            name: "Unsloth Studio".to_string(),
            base_url: "http://localhost:8001/v1".to_string(),
            wire_api: "responses".to_string(),
            requires_openai_auth: false,
            supports_websockets: false,
            default_model: None,
            model_catalog_json: None,
            env_key: None,
            request_max_retries: 0,
            stream_max_retries: 0,
        };
        let catalog = parse_model_catalog(r#"{"data":[{"id":"first"},{"id":"second"}]}"#).unwrap();
        let temp = tempfile::tempdir().unwrap();

        apply_model_catalog_to_profile(&mut profile, temp.path(), &catalog).unwrap();

        assert_eq!(profile.default_model.as_deref(), Some("first"));
        assert!(profile
            .model_catalog_json
            .as_ref()
            .is_some_and(|path| path.exists()));
    }
}
