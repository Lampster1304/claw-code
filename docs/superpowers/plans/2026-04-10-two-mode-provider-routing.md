# Two-Mode Provider Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert AGCLI provider routing to exactly two runtime modes (Local/Ollama + Cloud/OpenAI-compatible), with automatic local-first detection and removal of Anthropic/xAI/DashScope-specific routes.

**Architecture:** Collapse provider selection to `ProviderKind::{Local, Cloud}` in the `api` crate, then propagate that contract through `ProviderClient`, CLI runtime wiring, tools fallback tests, and docs. Keep existing Ollama transport and OpenAI-compatible transport, but remove provider-specific dispatch branches and env-var-specific routing heuristics tied to Anthropic/xAI/DashScope.

**Tech Stack:** Rust workspace (`cargo`), crates `api`, `tools`, `rusty-claude-cli`, Markdown docs.

---

## File Structure (planned edit map)

**Core provider routing**
- Modify: `rust/crates/api/src/providers/mod.rs` (ProviderKind, alias/metadata/routing logic, provider tests)
- Modify: `rust/crates/api/src/client.rs` (ProviderClient variants + construction + tests)
- Modify: `rust/crates/api/src/lib.rs` (re-exports aligned to two-mode contract)

**API tests**
- Modify: `rust/crates/api/tests/provider_client_integration.rs` (remove xAI/Anthropic-specific assumptions)
- Modify: `rust/crates/api/tests/openai_compat_integration.rs` (provider dispatch test to generic cloud route)
- Modify: `rust/crates/api/tests/client_integration.rs` (provider-dispatch integration test update)

**CLI runtime**
- Modify: `rust/crates/rusty-claude-cli/src/main.rs` (provider labels, runtime dispatch, auth doctor check, login/logout behavior, tests)

**Tools provider chain tests**
- Modify: `rust/crates/tools/src/lib.rs` (provider fallback tests move from Anthropic/xAI models to local/cloud models)

**Documentation**
- Modify: `README.md`
- Modify: `USAGE.md`
- Modify: `LOCAL_MODELS.md`

---

### Task 1: Collapse provider routing in `api` crate to Local + Cloud

**Files:**
- Modify: `rust/crates/api/src/providers/mod.rs:33-232, 492-1001`
- Modify: `rust/crates/api/src/client.rs:11-116, 144-155, 174-253`
- Modify: `rust/crates/api/src/lib.rs:9-27`
- Test: `rust/crates/api/src/providers/mod.rs` (unit tests in-module)
- Test: `rust/crates/api/src/client.rs` (unit tests in-module)

- [ ] **Step 1: Write failing routing tests for two-mode behavior**

```rust
// rust/crates/api/src/providers/mod.rs (tests module)
#[test]
fn detect_provider_kind_prefers_local_when_local_env_and_openai_key_both_set() {
    let _lock = env_lock();
    let _local_provider = EnvVarGuard::set("AGCLI_LOCAL_PROVIDER", Some("ollama"));
    let _openai_key = EnvVarGuard::set("OPENAI_API_KEY", Some("sk-test"));

    assert_eq!(detect_provider_kind("llama3.2"), ProviderKind::Local);
}

#[test]
fn detect_provider_kind_routes_openai_prefixed_models_to_cloud() {
    assert_eq!(detect_provider_kind("openai/gpt-4.1-mini"), ProviderKind::Cloud);
    assert_eq!(detect_provider_kind("gpt-4o-mini"), ProviderKind::Cloud);
}

#[test]
fn detect_provider_kind_defaults_to_local_without_cloud_credentials() {
    let _lock = env_lock();
    let _openai_key = EnvVarGuard::set("OPENAI_API_KEY", None);
    let _local_provider = EnvVarGuard::set("AGCLI_LOCAL_PROVIDER", None);
    let _ollama_host = EnvVarGuard::set("OLLAMA_HOST", None);

    assert_eq!(detect_provider_kind("unknown-model"), ProviderKind::Local);
}
```

- [ ] **Step 2: Run targeted tests to confirm initial failure**

Run:
```bash
cd rust
cargo test -p api detect_provider_kind_routes_openai_prefixed_models_to_cloud -- --exact
```

Expected: FAIL (because `ProviderKind::Cloud` and two-mode logic are not implemented yet).

- [ ] **Step 3: Implement two-mode provider core**

```rust
// rust/crates/api/src/providers/mod.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Local,
    Cloud,
}

#[must_use]
pub fn detect_provider_kind(model: &str) -> ProviderKind {
    let canonical = resolve_model_alias(model);

    if canonical.starts_with("ollama/") || canonical.starts_with("local/") {
        return ProviderKind::Local;
    }
    if ollama::local_provider_enabled() {
        return ProviderKind::Local;
    }
    if canonical.starts_with("openai/") || canonical.starts_with("gpt-") {
        return ProviderKind::Cloud;
    }
    if openai_compat::has_api_key("OPENAI_API_KEY") {
        return ProviderKind::Cloud;
    }
    ProviderKind::Local
}
```

```rust
// rust/crates/api/src/client.rs
#[derive(Debug, Clone)]
pub enum ProviderClient {
    Local(OllamaClient),
    Cloud(OpenAiCompatClient),
}

pub fn from_model_with_anthropic_auth(
    model: &str,
    _anthropic_auth: Option<AuthSource>,
) -> Result<Self, ApiError> {
    let resolved_model = providers::resolve_model_alias(model);
    match providers::detect_provider_kind(&resolved_model) {
        ProviderKind::Local => Ok(Self::Local(OllamaClient::from_model(&resolved_model))),
        ProviderKind::Cloud => Ok(Self::Cloud(OpenAiCompatClient::from_env(
            OpenAiCompatConfig::openai(),
        )?)),
    }
}
```

```rust
// rust/crates/api/src/lib.rs (exports)
pub use providers::{
    detect_provider_kind, max_tokens_for_model, max_tokens_for_model_with_override,
    resolve_model_alias, ProviderKind,
};
```

- [ ] **Step 4: Re-run api crate tests**

Run:
```bash
cd rust
cargo test -p api
```

Expected: PASS for updated `api` unit tests.

- [ ] **Step 5: Commit api two-mode routing changes**

```bash
cd /Users/berserk/Documents/CodeTerm/AGCLI
git add rust/crates/api/src/providers/mod.rs rust/crates/api/src/client.rs rust/crates/api/src/lib.rs
git commit -m "refactor(api): collapse provider routing to local and cloud"
```

---

### Task 2: Update API integration tests to new provider contract

**Files:**
- Modify: `rust/crates/api/tests/provider_client_integration.rs`
- Modify: `rust/crates/api/tests/openai_compat_integration.rs:312-348`
- Modify: `rust/crates/api/tests/client_integration.rs:461-499`
- Test: same files above

- [ ] **Step 1: Write failing integration tests for Local + Cloud dispatch**

```rust
// rust/crates/api/tests/provider_client_integration.rs
#[test]
fn provider_client_routes_openai_prefixed_model_to_cloud() {
    let _lock = env_lock();
    let _openai_api_key = EnvVarGuard::set("OPENAI_API_KEY", Some("openai-test-key"));

    let client =
        ProviderClient::from_model("openai/gpt-4.1-mini").expect("cloud route should resolve");
    assert_eq!(client.provider_kind(), ProviderKind::Cloud);
}

#[test]
fn provider_client_prefers_local_when_ollama_host_is_set() {
    let _lock = env_lock();
    let _openai_api_key = EnvVarGuard::set("OPENAI_API_KEY", Some("openai-test-key"));
    let _ollama_host = EnvVarGuard::set("OLLAMA_HOST", Some("http://127.0.0.1:11434"));

    let client = ProviderClient::from_model("llama3.2").expect("local route should resolve");
    assert_eq!(client.provider_kind(), ProviderKind::Local);
}
```

```rust
// rust/crates/api/tests/openai_compat_integration.rs
#[tokio::test]
async fn provider_client_dispatches_cloud_requests_from_openai_env() {
    let _lock = env_lock();
    let _api_key = ScopedEnvVar::set("OPENAI_API_KEY", "openai-test-key");
    let _base_url = ScopedEnvVar::set("OPENAI_BASE_URL", server.base_url());

    let client = ProviderClient::from_model("openai/gpt-4.1-mini")
        .expect("cloud provider client should be constructed");
    assert!(matches!(client, ProviderClient::Cloud(_)));
}
```

- [ ] **Step 2: Run targeted integration tests and confirm failure first**

Run:
```bash
cd rust
cargo test -p api provider_client_routes_openai_prefixed_model_to_cloud -- --exact
cargo test -p api provider_client_dispatches_cloud_requests_from_openai_env -- --exact
```

Expected: FAIL until test names/assertions are fully aligned with new enum variants and routing.

- [ ] **Step 3: Update existing integration assertions from xAI/Anthropic to Cloud/Local**

```rust
// rust/crates/api/tests/client_integration.rs
#[tokio::test]
async fn provider_client_dispatches_cloud_requests() {
    let state = Arc::new(Mutex::new(Vec::<CapturedRequest>::new()));
    let server = spawn_server(
        state.clone(),
        vec![http_response(
            "200 OK",
            "application/json",
            "{\"id\":\"msg_provider\",\"choices\":[{\"message\":{\"role\":\"assistant\",\"content\":\"Dispatched\"},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":3,\"completion_tokens\":2}}",
        )],
    )
    .await;
    std::env::set_var("OPENAI_API_KEY", "openai-test-key");
    std::env::set_var("OPENAI_BASE_URL", server.base_url());

    let client = ProviderClient::from_model("openai/gpt-4.1-mini")
        .expect("cloud provider should construct");
    let response = client
        .send_message(&sample_request(false))
        .await
        .expect("cloud provider request should succeed");

    assert_eq!(response.total_tokens(), 5);
    let captured = state.lock().await;
    let request = captured.first().expect("server should capture request");
    assert_eq!(request.path, "/chat/completions");
    assert_eq!(
        request.headers.get("authorization").map(String::as_str),
        Some("Bearer openai-test-key")
    );
}
```

```rust
// rust/crates/api/tests/provider_client_integration.rs imports
use api::{ApiError, ProviderClient, ProviderKind};
```

- [ ] **Step 4: Re-run API integration tests**

Run:
```bash
cd rust
cargo test -p api --test provider_client_integration
cargo test -p api --test openai_compat_integration
cargo test -p api --test client_integration
```

Expected: PASS.

- [ ] **Step 5: Commit API integration test updates**

```bash
cd /Users/berserk/Documents/CodeTerm/AGCLI
git add rust/crates/api/tests/provider_client_integration.rs rust/crates/api/tests/openai_compat_integration.rs rust/crates/api/tests/client_integration.rs
git commit -m "test(api): migrate provider integration coverage to local and cloud"
```

---

### Task 3: Align CLI runtime/provider UX to two-mode contract

**Files:**
- Modify: `rust/crates/rusty-claude-cli/src/main.rs:1127-1139, 1570-1705, 2075-2180, 6786-6870, 10035-10079`
- Test: `rust/crates/rusty-claude-cli/src/main.rs` (in-file unit tests)

- [ ] **Step 1: Add failing CLI tests for provider labels and login/logout behavior**

```rust
#[test]
fn format_connected_line_renders_cloud_provider_for_openai_model() {
    let line = format_connected_line("openai/gpt-4.1-mini");
    assert_eq!(line, "Connected: openai/gpt-4.1-mini via cloud");
}

#[test]
fn check_auth_health_cloud_mode_reports_openai_key_status() {
    let _guard = env_lock();
    std::env::remove_var("AGCLI_LOCAL_PROVIDER");
    std::env::remove_var("OLLAMA_HOST");
    std::env::set_var("OPENAI_API_KEY", "openai-test-key");

    let check = check_auth_health();
    assert_eq!(check.name, "Auth");
    assert!(check.summary.contains("cloud"));
    assert!(check.details.iter().any(|detail| detail.contains("openai_api_key=present")));
}

#[test]
fn run_login_is_disabled_in_two_mode_provider_architecture() {
    let error = run_login(CliOutputFormat::Text).expect_err("login should be disabled");
    assert!(error.to_string().contains("disabled"));
}
```

- [ ] **Step 2: Run targeted CLI tests and confirm failure**

Run:
```bash
cd rust
cargo test -p rusty-claude-cli format_connected_line_renders_cloud_provider_for_openai_model -- --exact
```

Expected: FAIL until `ProviderKind::Cloud` and label mapping are implemented in CLI.

- [ ] **Step 3: Implement CLI two-mode behavior**

```rust
fn provider_label(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Local => "local",
        ProviderKind::Cloud => "cloud",
    }
}
```

```rust
// In runtime client construction branch
let client = match provider_kind {
    ProviderKind::Local => ApiProviderClient::from_model_with_anthropic_auth(&resolved_model, None)?,
    ProviderKind::Cloud => ApiProviderClient::from_model_with_anthropic_auth(&resolved_model, None)?,
};
```

```rust
fn check_auth_health() -> DiagnosticCheck {
    if effective_provider_is_local() {
        return DiagnosticCheck::new(
            "Auth",
            DiagnosticLevel::Ok,
            "local provider active — no cloud credentials required",
        );
    }

    let openai_key_present = env::var("OPENAI_API_KEY")
        .ok()
        .is_some_and(|value| !value.trim().is_empty());
    DiagnosticCheck::new(
        "Auth",
        if openai_key_present {
            DiagnosticLevel::Ok
        } else {
            DiagnosticLevel::Warn
        },
        if openai_key_present {
            "cloud mode configured via OPENAI_API_KEY"
        } else {
            "cloud mode selected but OPENAI_API_KEY is missing"
        },
    )
    .with_details(vec![format!(
        "Environment       openai_api_key={}",
        if openai_key_present { "present" } else { "absent" }
    )])
}
```

```rust
fn run_login(_output_format: CliOutputFormat) -> Result<(), Box<dyn std::error::Error>> {
    Err(io::Error::other("login is disabled in two-mode provider architecture (local/cloud openai-compatible)").into())
}

fn run_logout(_output_format: CliOutputFormat) -> Result<(), Box<dyn std::error::Error>> {
    Err(io::Error::other("logout is disabled in two-mode provider architecture (local/cloud openai-compatible)").into())
}
```

- [ ] **Step 4: Run CLI crate tests**

Run:
```bash
cd rust
cargo test -p rusty-claude-cli
```

Expected: PASS for updated provider/doctor/login assertions.

- [ ] **Step 5: Commit CLI two-mode changes**

```bash
cd /Users/berserk/Documents/CodeTerm/AGCLI
git add rust/crates/rusty-claude-cli/src/main.rs
git commit -m "refactor(cli): enforce two-mode local/cloud provider behavior"
```

---

### Task 4: Update tools provider fallback tests for two-mode models

**Files:**
- Modify: `rust/crates/tools/src/lib.rs:8329-8471`
- Test: `rust/crates/tools/src/lib.rs` (in-file tests)

- [ ] **Step 1: Write failing fallback-chain tests using local/cloud models**

```rust
#[test]
fn provider_runtime_client_chain_appends_cloud_fallbacks_in_order() {
    let _guard = env_lock().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let original_openai = std::env::var_os("OPENAI_API_KEY");
    std::env::set_var("OPENAI_API_KEY", "openai-test-key");
    let fallback_config = ProviderFallbackConfig::new(
        None,
        vec!["openai/gpt-4.1-mini".to_string(), "openai/gpt-4.1".to_string()],
    );

    let client = ProviderRuntimeClient::new_with_fallback_config(
        "ollama/qwen3:8b".to_string(),
        BTreeSet::new(),
        &fallback_config,
    )
    .expect("chain with cloud fallbacks should construct");

    assert_eq!(client.chain[0].model, "ollama/qwen3:8b");
    assert_eq!(client.chain[1].model, "openai/gpt-4.1-mini");
    assert_eq!(client.chain[2].model, "openai/gpt-4.1");
}
```

- [ ] **Step 2: Run tools tests and confirm initial failure**

Run:
```bash
cd rust
cargo test -p tools provider_runtime_client_chain_appends_cloud_fallbacks_in_order -- --exact
```

Expected: FAIL until old anthropic/xai-based fixtures are replaced.

- [ ] **Step 3: Replace Anthropic/xAI fallback fixtures with Local/Cloud fixtures**

```rust
// rust/crates/tools/src/lib.rs
let fallback_config = ProviderFallbackConfig::new(
    Some("openai/gpt-4.1-mini".to_string()),
    vec!["ollama/qwen3:8b".to_string()],
);

let client = ProviderRuntimeClient::new_with_fallback_config(
    "ollama/qwen3:8b".to_string(),
    BTreeSet::new(),
    &fallback_config,
)
.expect("chain with cloud primary should construct");

assert_eq!(client.chain[0].model, "openai/gpt-4.1-mini");
assert_eq!(client.chain[1].model, "ollama/qwen3:8b");
```

- [ ] **Step 4: Re-run tools crate tests**

Run:
```bash
cd rust
cargo test -p tools
```

Expected: PASS.

- [ ] **Step 5: Commit tools fallback test migration**

```bash
cd /Users/berserk/Documents/CodeTerm/AGCLI
git add rust/crates/tools/src/lib.rs
git commit -m "test(tools): migrate provider fallback fixtures to local and cloud"
```

---

### Task 5: Rewrite docs to two-mode provider story

**Files:**
- Modify: `README.md`
- Modify: `USAGE.md`
- Modify: `LOCAL_MODELS.md`

- [ ] **Step 1: Update README quick-start and auth text**

```markdown
AGCLI supports two provider modes:
1. Local mode (Ollama/local server)
2. Cloud mode (OpenAI-compatible API gateway)

Routing is automatic with local-first precedence.
```

- [ ] **Step 2: Update USAGE.md provider section and remove Anthropic/xAI/DashScope routing matrix**

```markdown
## Provider modes

### Local mode
- `AGCLI_LOCAL_PROVIDER=ollama` or `OLLAMA_HOST`
- default fallback when cloud credentials are absent

### Cloud mode (OpenAI-compatible)
- `OPENAI_API_KEY`
- optional `OPENAI_BASE_URL`
- supports gateway routing for Copilot/Gemini-compatible endpoints
```

- [ ] **Step 3: Update LOCAL_MODELS.md to document local-first autodetection and cloud fallback**

```markdown
Autodetection order:
1. explicit local model prefix (`ollama/`, `local/`)
2. local env (`AGCLI_LOCAL_PROVIDER`, `OLLAMA_HOST`)
3. explicit cloud model prefix (`openai/`, `gpt-`)
4. cloud env (`OPENAI_API_KEY`)
5. fallback to local
```

- [ ] **Step 4: Run docs consistency checks**

Run:
```bash
cd /Users/berserk/Documents/CodeTerm/AGCLI
rg -n "Anthropic|xAI|DashScope|ANTHROPIC_|XAI_|DASHSCOPE_" README.md USAGE.md LOCAL_MODELS.md
```

Expected: no routing guidance for removed provider-specific paths in these three docs.

- [ ] **Step 5: Commit doc rewrite**

```bash
cd /Users/berserk/Documents/CodeTerm/AGCLI
git add README.md USAGE.md LOCAL_MODELS.md
git commit -m "docs: document two-mode provider routing with local-first autodetection"
```

---

### Task 6: Final regression run for touched crates

**Files:**
- Modify: none expected; if regressions appear, modify only files already touched in Tasks 1-5
- Test: full crate runs for `api`, `tools`, `rusty-claude-cli`

- [ ] **Step 1: Run final targeted crate suites**

Run:
```bash
cd rust
cargo test -p api && cargo test -p tools && cargo test -p rusty-claude-cli
```

Expected: PASS.

- [ ] **Step 2: Run workspace-level sanity**

Run:
```bash
cd rust
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 3: Stage any final fixups**

```bash
cd /Users/berserk/Documents/CodeTerm/AGCLI
git add rust/crates/api/src/providers/mod.rs rust/crates/api/src/client.rs rust/crates/api/src/lib.rs \
        rust/crates/api/tests/provider_client_integration.rs rust/crates/api/tests/openai_compat_integration.rs \
        rust/crates/api/tests/client_integration.rs rust/crates/tools/src/lib.rs \
        rust/crates/rusty-claude-cli/src/main.rs README.md USAGE.md LOCAL_MODELS.md
```

- [ ] **Step 4: Create final integration commit when staged changes exist**

```bash
cd /Users/berserk/Documents/CodeTerm/AGCLI
git diff --cached --quiet || git commit -m "feat: finalize two-mode provider routing across api cli tools and docs"
```

Expected: either a new final commit or "nothing to commit" if prior commits already cover all changes.
