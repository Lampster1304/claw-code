# Two-Mode Provider Routing Design (Local + Cloud)

Date: 2026-04-10
Project: AGCLI (`rust/` workspace)
Status: Approved for implementation

## Problem

AGCLI currently carries multiple provider-specific routes (Anthropic, xAI, DashScope, OpenAI-compat, Local/Ollama). The desired product shape is simpler:

1. Local mode for local models.
2. Cloud mode for a generic OpenAI-compatible API endpoint.

The CLI must auto-detect mode using model names and credentials, with local-first behavior for ambiguous cases.

## Goals

1. Keep only two runtime provider routes: `Local` and `Cloud`.
2. Use automatic routing based on model/credentials.
3. Prefer local routing when ambiguous.
4. Remove provider-specific routing branches for Anthropic, xAI, and DashScope.

## Non-goals

1. Add native Gemini or GitHub Models protocol clients in this change.
2. Redesign all command surfaces.
3. Refactor unrelated runtime/tooling systems.

## Selected Approach

Approach B (selected): simplify the provider architecture to two concrete provider kinds and update routing, CLI behavior, tests, and docs accordingly.

## Architecture

### Provider kinds

`ProviderKind` becomes:

1. `Local` (Ollama/local model server)
2. `Cloud` (OpenAI-compatible endpoint configured by env vars)

### Routing algorithm (auto, local-first)

Given a model name and environment:

1. If model starts with `ollama/` or `local/` => `Local`
2. Else if `AGCLI_LOCAL_PROVIDER=ollama` or `OLLAMA_HOST` is set => `Local`
3. Else if model starts with `openai/` or `gpt-` => `Cloud`
4. Else if `OPENAI_API_KEY` is set => `Cloud`
5. Else fallback => `Local`

This preserves local-first behavior in ambiguous cases.

## Component-level changes

### `api` crate

1. Collapse provider metadata/routing from many cloud providers to a single `Cloud` branch.
2. Remove Anthropic/xAI/DashScope-specific routing branches in `providers/mod.rs` and `client.rs`.
3. Keep `OllamaClient` for local path and `OpenAiCompatClient` for cloud path.
4. Update tests to validate only `Local` and `Cloud` routing semantics.

### `rusty-claude-cli` crate

1. Runtime client construction uses only `Local` or `Cloud`.
2. `format_connected_line` prints only `via local` or `via cloud`.
3. `doctor` auth checks report local/cloud status and remove Anthropic OAuth assumptions.
4. `login`/`logout` commands return a clear "unsupported in current two-mode architecture" error.

### `tools` crate

1. Provider fallback chain remains available but now resolves against two-mode provider semantics.
2. Tests using provider-specific models are updated to two-mode equivalents.

## Error handling

1. Unsupported/legacy provider-specific inputs should fail with explicit actionable errors.
2. Cloud-mode missing credentials should point to `OPENAI_API_KEY` and optional `OPENAI_BASE_URL`.
3. Local-mode configuration errors should continue to reference `AGCLI_LOCAL_PROVIDER`, `AGCLI_LOCAL_BASE_URL`, and `OLLAMA_HOST`.

## Testing plan

1. Update unit tests in `api` for provider detection and alias routing.
2. Update runtime/CLI tests that assert Anthropic/xAI labels.
3. Update tool tests that rely on multi-provider fallback assumptions.
4. Run existing crate test suites for:
   - `api`
   - `tools`
   - `rusty-claude-cli`

## Documentation updates

1. Update root `README.md` and `USAGE.md` to explain only two modes.
2. Remove or rewrite Anthropic/xAI/DashScope route examples.
3. Keep "Cloud OpenAI-compatible gateway" guidance for Copilot/Gemini gateway setups.

## Risks and mitigations

1. Risk: legacy configs referencing Anthropic/xAI models break.
   - Mitigation: return explicit migration errors and examples.
2. Risk: tests tightly coupled to prior provider labels fail.
   - Mitigation: systematically update expected labels and env var fixtures.
3. Risk: accidental behavior drift in local default path.
   - Mitigation: preserve local-first routing order and add explicit tests for ambiguity resolution.
