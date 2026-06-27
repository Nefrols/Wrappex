use crate::models::fetch_model_ids;
use crate::profile::{derive_profile_id, validate_profile_id, Profile, ProfileStore};
use anyhow::{anyhow, Result};
use inquire::{Confirm, Select, Text};

const CREATE_PROFILE: &str = "Create profile";

pub fn print_profiles(store: &ProfileStore) {
    for profile in &store.profiles {
        let model = profile
            .default_model
            .as_deref()
            .unwrap_or("<no default model>");
        println!(
            "{} ({}) - {} - {}",
            profile.name, profile.id, profile.base_url, model
        );
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
        let id_start = selected
            .rfind('(')
            .ok_or_else(|| anyhow!("invalid selection"))?
            + 1;
        let id_end = selected
            .rfind(')')
            .ok_or_else(|| anyhow!("invalid selection"))?;
        let id = &selected[id_start..id_end];
        store
            .profiles
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
    let wire_api = Text::new("Wire API").with_default("responses").prompt()?;
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
            let message = format!("{env_key} is not set. Continue?");
            let proceed = Confirm::new(&message).with_default(false).prompt()?;
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
    let message = format!("Remove profile {}?", profile.name);
    if Confirm::new(&message).with_default(false).prompt()? {
        store.profiles.remove(index);
    }
    Ok(())
}
