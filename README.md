# AGCLI

<p align="center">
  <a href="https://github.com/ultraworkers/claw-code">ultraworkers/claw-code</a>
  ·
  <a href="./USAGE.md">Usage</a>
  ·
  <a href="./rust/README.md">Rust workspace</a>
  ·
  <a href="./PARITY.md">Parity</a>
  ·
  <a href="./ROADMAP.md">Roadmap</a>
  ·
  <a href="https://discord.gg/5TUQKqFWd">UltraWorkers Discord</a>
</p>

AGCLI is a local-first Rust agent CLI for coding workflows.

AGCLI supports exactly two provider modes:

1. **Local mode** (Ollama or another local server)
2. **Cloud mode** (OpenAI-compatible API gateway)

Routing is automatic with **local-first autodetection**.

> [!IMPORTANT]
> Start with [`LOCAL_MODELS.md`](./LOCAL_MODELS.md) and [`USAGE.md`](./USAGE.md) for provider setup and routing behavior.

## Quick start

```bash
# 1. Clone and build
git clone https://github.com/ultraworkers/claw-code
cd claw-code/rust
cargo build --workspace

# 2a. Local mode (Ollama)
export AGCLI_LOCAL_PROVIDER=ollama
export AGCLI_LOCAL_BASE_URL=http://127.0.0.1:11434

# 2b. Cloud mode (OpenAI-compatible gateway)
export OPENAI_API_KEY="your-key"
# Optional for compatible gateways (OpenRouter, Copilot-compatible, Gemini-compatible, etc.)
# export OPENAI_BASE_URL="https://your-gateway.example/v1"

# 3. Verify everything is wired correctly
./target/debug/agcli doctor

# 4. Run a prompt
./target/debug/agcli prompt "say hello"
```

Run the workspace test suite:

```bash
cd rust
cargo test --workspace
```

## Documentation map

- [`USAGE.md`](./USAGE.md) — commands, auth, sessions, config, and provider modes
- [`LOCAL_MODELS.md`](./LOCAL_MODELS.md) — local-mode setup and local-first routing order
- [`rust/README.md`](./rust/README.md) — crate map, CLI surface, features, workspace layout
- [`PARITY.md`](./PARITY.md) — parity status for the Rust port
- [`ROADMAP.md`](./ROADMAP.md) — active roadmap and open cleanup work
- [`PHILOSOPHY.md`](./PHILOSOPHY.md) — why the project exists and how it is operated

## Ownership / affiliation disclaimer

- This repository does **not** claim ownership of the original Claude Code source material.
- This repository is **not affiliated with, endorsed by, or maintained by upstream model/API vendors**.
