# wrappex

`wrappex` launches the original Codex CLI with local OpenAI-compatible model provider profiles stored in `~/.wrappex/profiles.toml`.

Model metadata from the provider `/models` endpoint is cached as Codex-compatible catalogs under `~/.wrappex/model-catalogs/` and passed to Codex with `model_catalog_json`.

Wrappex does not prompt for a model on startup. It uses the profile `default_model` when set; otherwise it refreshes `/models`, stores the first returned model as the profile default, and starts Codex with that model. If no model can be resolved, Codex starts without `--model` so you can choose one inside Codex with `/model`.

## Build

```powershell
cargo build --release
```

## Install

```powershell
.\scripts\install.ps1 -AddToPath
```

Restart the terminal after the first install so the updated user `PATH` is loaded.

## Usage

```powershell
wrappex
wrappex profile create
wrappex profile list
wrappex run unsloth-local -- --sandbox workspace-write
```

Set `WRAPPEX_CODEX_BIN` or pass `--codex-bin` if `codex` is not next to `wrappex` and not on `PATH`.
