use crate::profile::Profile;
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::Value;
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
    id: Option<Value>,
}

pub fn parse_model_ids(raw: &str) -> Result<Vec<String>, ModelError> {
    let response: ModelsResponse = serde_json::from_str(raw).map_err(ModelError::Json)?;
    let mut ids: Vec<String> = response
        .data
        .into_iter()
        .filter_map(|item| match item.id {
            Some(Value::String(id)) => Some(id),
            _ => None,
        })
        .collect();
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
