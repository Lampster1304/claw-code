# AGCLI Usage

This guide covers the Rust workspace under `rust/` and the `agcli` binary.

## Provider modes (two-mode architecture)

AGCLI supports exactly two provider modes:

1. **Local** — Ollama or another local server
2. **Cloud** — OpenAI-compatible API gateway

Routing is automatic and **local-first**.

## Local-first autodetection order

When AGCLI resolves a model, routing follows this order:

1. Explicit local model prefix (`ollama/`, `local/`)
2. Local environment (`AGCLI_LOCAL_PROVIDER`, `OLLAMA_HOST`)
3. Explicit cloud model prefix (`openai/`, `gpt-`)
4. Cloud credentials (`OPENAI_API_KEY`)
5. Fallback to local mode

## Prerequisites

- Rust toolchain with `cargo`
- For local mode: local model server (for example Ollama)
- For cloud mode: `OPENAI_API_KEY`

## Build

```bash
cd rust
cargo build --workspace
```

## First-run doctor check

```bash
cd rust
./target/debug/agcli
/doctor
```

## Quick examples

### Interactive REPL

```bash
cd rust
./target/debug/agcli
```

### Interactive output behavior

- In interactive REPL mode, AGCLI shows live thinking by default.
- Use `--hide-thinking` to collapse/hide thinking output in REPL.
- `--output-format json` and `--compact` remain machine-friendly.

### One-shot prompt

```bash
cd rust
./target/debug/agcli prompt "summarize this repository"
```

### JSON output

```bash
cd rust
./target/debug/agcli --output-format json prompt "status"
```

## Local mode setup (Ollama/local server)

```bash
export AGCLI_LOCAL_PROVIDER=ollama
export AGCLI_LOCAL_BASE_URL=http://127.0.0.1:11434
# or rely on OLLAMA_HOST

cd rust
./target/debug/agcli --model "ollama/llama3.2" prompt "say ready"
```

## Cloud mode setup (OpenAI-compatible gateway)

```bash
export OPENAI_API_KEY="your-key"
# optional gateway override
export OPENAI_BASE_URL="https://your-gateway.example/v1"

cd rust
./target/debug/agcli --model "openai/gpt-4.1-mini" prompt "say ready"
```

Use this cloud mode for compatible gateways, including Copilot-compatible and Gemini-compatible endpoints, by setting `OPENAI_API_KEY` and (when needed) `OPENAI_BASE_URL`.

## Model and permission controls

```bash
cd rust
./target/debug/agcli --model ollama/qwen2.5-coder:7b prompt "review this diff"
./target/debug/agcli --permission-mode plan-mode prompt "summarize Cargo.toml"
./target/debug/agcli --permission-mode auto-accepts-edits prompt "update README.md"
./target/debug/agcli --allowedTools read,glob "inspect the runtime crate"
```

Supported permission modes:

- `plan-mode` (aliases: `plan`, `read-only`)
- `auto-accepts-edits` (aliases: `acceptEdits`, `auto`, `workspace-write`)
- `danger-full-access`

## Session management

```bash
cd rust
./target/debug/agcli --resume latest
./target/debug/agcli --resume latest /status /diff
```

## Verification

```bash
cd rust
cargo test --workspace
```
