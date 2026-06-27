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
