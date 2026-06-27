use crate::cli::{Cli, Command as CliCommand, ProfileCommand};
use crate::profile::{load_profiles, save_profiles, Profile, ProfileStore};
use anyhow::{anyhow, Result};
use clap::Parser;
use std::path::PathBuf;

pub fn default_profiles_path() -> Result<PathBuf> {
    Ok(default_wrappex_dir()?.join("profiles.toml"))
}

pub fn default_wrappex_dir() -> Result<PathBuf> {
    let home = env_home_dir().ok_or_else(|| anyhow!("could not determine home directory"))?;
    Ok(home.join(".wrappex"))
}

fn env_home_dir() -> Option<PathBuf> {
    ["HOME", "USERPROFILE"]
        .into_iter()
        .filter_map(|key| std::env::var_os(key))
        .find(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(dirs::home_dir)
}

pub fn find_profile<'a>(store: &'a ProfileStore, query: &str) -> Option<&'a Profile> {
    let normalized = query.to_ascii_lowercase();
    store
        .profiles
        .iter()
        .find(|profile| profile.id == query || profile.name.to_ascii_lowercase() == normalized)
}

pub fn run_from_env() -> Result<()> {
    let cli = Cli::parse();
    let wrappex_dir = default_wrappex_dir()?;
    let profiles_path = wrappex_dir.join("profiles.toml");
    let mut store = load_profiles(&profiles_path)?;

    match cli.command {
        Some(CliCommand::Profile(args)) => match args.command {
            ProfileCommand::List => {
                crate::ui::print_profiles(&store);
                Ok(())
            }
            ProfileCommand::Create => {
                let mut profile = crate::ui::prompt_new_profile()?;
                refresh_catalog_or_warn(&mut profile, &wrappex_dir);
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
            let mut profile = find_profile(&store, &args.profile)
                .ok_or_else(|| anyhow!("profile '{}' not found", args.profile))?
                .clone();
            refresh_catalog_or_warn(&mut profile, &wrappex_dir);
            upsert_profile(&mut store, profile.clone());
            save_profiles(&profiles_path, &store)?;
            run_profile(cli.codex_bin, profile, args.codex_args)
        }
        None => {
            let mut profile = crate::ui::select_or_create_profile(&mut store)?;
            refresh_catalog_or_warn(&mut profile, &wrappex_dir);
            upsert_profile(&mut store, profile.clone());
            save_profiles(&profiles_path, &store)?;
            run_profile(cli.codex_bin, profile, Vec::new())
        }
    }
}

fn refresh_catalog_or_warn(profile: &mut Profile, wrappex_dir: &std::path::Path) {
    if let Err(error) = crate::models::refresh_profile_model_catalog(profile, wrappex_dir) {
        eprintln!("wrappex: could not refresh model metadata: {error}");
        if profile
            .model_catalog_json
            .as_ref()
            .is_some_and(|path| !path.exists())
        {
            profile.model_catalog_json = None;
        }
    }
}

fn upsert_profile(store: &mut ProfileStore, profile: Profile) {
    if let Some(existing) = store
        .profiles
        .iter_mut()
        .find(|existing| existing.id == profile.id)
    {
        *existing = profile;
    } else {
        store.profiles.push(profile);
    }
}

fn run_profile(
    codex_bin: Option<PathBuf>,
    profile: Profile,
    passthrough: Vec<String>,
) -> Result<()> {
    crate::ui::warn_missing_env_key(&profile)?;
    let codex_bin = crate::launch::resolve_codex_bin_from_env(codex_bin)?;
    let args =
        crate::launch::build_codex_args(&profile, profile.default_model.as_deref(), &passthrough);
    let code = crate::launch::run_codex(&codex_bin, &args)?;
    std::process::exit(code);
}

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
                model_catalog_json: None,
                env_key: None,
                request_max_retries: 0,
                stream_max_retries: 0,
            }],
        };

        assert_eq!(
            find_profile(&store, "unsloth-local").unwrap().name,
            "Unsloth Studio"
        );
        assert_eq!(
            find_profile(&store, "unsloth studio").unwrap().id,
            "unsloth-local"
        );
    }
}
