# Wrappex Design

## Goal

Build `wrappex`, a standalone Rust CLI wrapper for the existing `codex` binary. The user can place the compiled `wrappex` binary next to `codex` in a bin directory, run `wrappex` from any project folder, choose or create a local-model profile, optionally choose a model exposed by the provider API, and then hand off to the original Codex TUI with the correct model provider overrides.

## Constraints and Source Facts

- `wrappex` is a separate project in `E:\CURRENT\_qodex`; it does not patch `E:\CURRENT\codex`.
- Profiles are owned by `wrappex` and stored under `~/.wrappex`, not in `~/.codex/config.toml`.
- Current Codex CLI accepts `--model`/`-m` through shared options and accepts `-c key=value`/`--config key=value` runtime overrides with dotted TOML paths.
- Current Codex provider config supports `model_provider`, `model_providers.<id>.name`, `base_url`, `wire_api`, `env_key`, `requires_openai_auth`, retry settings, and websocket support.
- Unsloth Studio and similar local servers expose OpenAI-compatible endpoints; model discovery should call `<base_url>/models` and parse the standard OpenAI-compatible `data[].id` response.

## User Experience

Running `wrappex` without flags opens an interactive terminal menu:

1. Existing profiles are listed by display name, provider id, base URL, and default model if present.
2. The menu includes `Create profile`.
3. Selecting a profile refreshes available models from `/models` when possible.
4. If models are found, the user chooses one; if the profile has a default model, it is preselected.
5. If model discovery fails, the user can continue with the stored/default model or type a model manually.
6. `wrappex` launches the original `codex` in the current working directory.

The create-profile wizard asks for:

- Profile display name.
- Stable provider id, derived from the name by default and editable.
- Base URL, defaulting to `http://localhost:8001/v1`.
- Wire API, defaulting to `responses`.
- Whether auth is needed. Default is no auth for local providers.
- Optional API key environment variable name, if the provider needs a bearer token.
- Optional default model, preferably selected from `/models`.

## CLI Shape

Initial command surface:

- `wrappex`: interactive profile picker and launch.
- `wrappex run <profile-id-or-name>`: skip profile picker and still show model selection before launch.
- `wrappex profile create`: run the create-profile wizard.
- `wrappex profile list`: print configured profiles.
- `wrappex profile remove <profile-id-or-name>`: remove a saved profile after confirmation.
- `wrappex --codex-bin <path>`: override how the original Codex binary is found.

Any arguments after `--` are passed through to `codex`. Example: `wrappex run unsloth -- --sandbox workspace-write`.

## Profile File

Profiles live in `~/.wrappex/profiles.toml`.

```toml
version = 1

[[profiles]]
id = "unsloth-local"
name = "Unsloth Studio"
base_url = "http://localhost:8001/v1"
wire_api = "responses"
requires_openai_auth = false
supports_websockets = false
default_model = "unsloth/Qwen3-Coder-30B-A3B-Instruct"
env_key = ""
request_max_retries = 0
stream_max_retries = 0
```

`env_key` is omitted or empty when no provider token is needed. `request_max_retries` and `stream_max_retries` default to `0` for local development to surface provider issues quickly.

## Launch Contract

For a selected profile and model, `wrappex` executes the original `codex` with explicit runtime overrides:

```text
codex --model <model> \
  -c model_provider=<profile.id> \
  -c model_providers.<profile.id>.name=<profile.name> \
  -c model_providers.<profile.id>.base_url=<profile.base_url> \
  -c model_providers.<profile.id>.wire_api=<profile.wire_api> \
  -c model_providers.<profile.id>.requires_openai_auth=false \
  -c model_providers.<profile.id>.supports_websockets=false \
  -c model_providers.<profile.id>.request_max_retries=0 \
  -c model_providers.<profile.id>.stream_max_retries=0
```

When `env_key` is set, `wrappex` adds:

```text
-c model_providers.<profile.id>.env_key=<env_key>
```

The wrapper does not add `--oss` or `--local-provider`, because it provides the full model provider definition directly. The child process inherits stdin, stdout, stderr, cwd, and environment. On Unix, `wrappex` should replace itself with Codex via `exec`; on Windows, it should spawn Codex and exit with the child exit code.

## Codex Binary Resolution

Resolution order:

1. `--codex-bin <path>`.
2. `WRAPPEX_CODEX_BIN`.
3. A sibling executable named `codex` or `codex.exe` next to the running `wrappex` binary.
4. `codex` found on `PATH`, excluding the `wrappex` binary itself.

If resolution fails, print a concise error with the searched locations.

## Model Discovery

`wrappex` calls `GET <base_url>/models` with a short timeout. If `env_key` is configured and present in the environment, it sends `Authorization: Bearer <value>`.

Accepted response shape:

```json
{
  "data": [
    { "id": "model-name" }
  ]
}
```

The model list is sorted by id. Empty, invalid, or unreachable responses become non-fatal warnings in interactive mode. The user can still type a model manually.

## Error Handling

- Invalid profile file: show parse error and path; do not overwrite automatically.
- Duplicate profile ids: reject on save.
- Invalid provider id: require lowercase letters, digits, `_`, and `-`, starting with a letter.
- Missing default model and failed discovery: prompt for manual model.
- Missing configured `env_key`: warn before launch and let the user continue or cancel.
- Codex child failure: exit with the same code when available, otherwise `1`.

## Testing Strategy

Use test-first Rust unit tests around the non-interactive core:

- Profile TOML load/save round trip.
- Provider id validation and name-to-id derivation.
- Codex argument construction, including optional `env_key`.
- Codex binary resolution with injected fake paths.
- OpenAI-compatible model response parsing.
- Launch exit-code mapping through a small command abstraction.

The interactive prompts should stay thin and call tested core functions. End-to-end manual verification will build the binary, run `wrappex profile create` against a local or mocked `/v1/models` endpoint, and verify the generated `codex` command path without requiring a real model turn.

## Out of Scope

- Editing `~/.codex/config.toml`.
- Managing or starting Unsloth Studio itself.
- Storing raw API keys in `~/.wrappex`.
- Supporting non-OpenAI-compatible model APIs in the first version.
- Reimplementing Codex login, sessions, or TUI behavior.
