# Interactive Thinking Terminal UX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make AGCLI clearly responsive in interactive REPL mode by showing live thinking by default, adding a clear input boundary (`› Tu mensaje`), and emitting a 10s waiting notice, while keeping `--json` and `--compact` behavior unchanged.

**Architecture:** Keep changes concentrated in `rust/crates/rusty-claude-cli/src/main.rs`. Parse a new `--hide-thinking` toggle, apply it to REPL runs, and configure stream rendering in `AnthropicRuntimeClient` (`emit_output` + `show_thinking`). Reuse existing streaming and rendering flow, adding focused helpers for thinking deltas, waiting notices, and REPL input boundaries.

**Tech Stack:** Rust, tokio async stream handling, crossterm terminal rendering, cargo test

---

## File structure and responsibilities

- Modify: `rust/crates/rusty-claude-cli/src/main.rs`
  - CLI flag parsing and help text (`parse_args`, usage/help rendering).
  - REPL prompt/boundary rendering (`run_repl`).
  - Turn execution UX (`run_turn` spinner label).
  - Stream rendering (`AnthropicRuntimeClient::consume_stream`, `push_output_block`, `response_to_events`).
  - New unit tests for flag parsing, thinking visibility, wait-notice threshold, and prompt boundary formatting.
- Modify: `USAGE.md`
  - Document default live-thinking behavior and `--hide-thinking`.

No new source files are required.

### Task 1: CLI flag plumbing (`--hide-thinking`) and help text

**Files:**
- Modify: `rust/crates/rusty-claude-cli/src/main.rs` (around `parse_args`, `run`, `run_repl`, `LiveCli`, `print_help_to`, unit tests)
- Test: `rust/crates/rusty-claude-cli/src/main.rs` (existing `#[cfg(test)]` module)

- [ ] **Step 1: Write failing tests for flag acceptance and help output**

```rust
#[test]
fn parses_hide_thinking_flag_without_changing_action_shape() {
    let _guard = env_lock();
    let repl = parse_args(&["--hide-thinking".to_string()]).expect("args should parse");
    assert!(matches!(repl, CliAction::Repl { .. }));

    let prompt = parse_args(&["--hide-thinking".to_string(), "hello".to_string()])
        .expect("prompt args should parse");
    assert!(matches!(prompt, CliAction::Prompt { .. }));
}

#[test]
fn help_lists_hide_thinking_flag() {
    let mut help = Vec::new();
    print_help_to(&mut help).expect("help should render");
    let help = String::from_utf8(help).expect("help should be utf8");
    assert!(help.contains("--hide-thinking"));
}
```

- [ ] **Step 2: Run targeted tests to confirm failure**

Run:
```bash
cd rust
cargo test -p rusty-claude-cli parses_hide_thinking_flag_without_changing_action_shape help_lists_hide_thinking_flag
```

Expected: FAIL (`unknown option: --hide-thinking` and missing help text assertion).

- [ ] **Step 3: Implement flag parsing and propagation**

```rust
// CLI_OPTION_SUGGESTIONS
const CLI_OPTION_SUGGESTIONS: &[&str] = &[
    "--help",
    "-h",
    "--version",
    "-V",
    "--model",
    "--output-format",
    "--permission-mode",
    "--dangerously-skip-permissions",
    "--allowedTools",
    "--allowed-tools",
    "--resume",
    "--print",
    "--compact",
    "--hide-thinking",
    "--base-commit",
    "-p",
];

// parse_args
            "--hide-thinking" => {
                index += 1;
            }

// run
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let hide_thinking = args.iter().any(|arg| arg == "--hide-thinking");
    // Prompt action branch:
    let mut cli = LiveCli::new(model, true, allowed_tools, permission_mode, false)?;
    // REPL action branch:
    run_repl(
        model,
        allowed_tools,
        permission_mode,
        base_commit,
        reasoning_effort,
        allow_broad_cwd,
        hide_thinking,
    )?;
    Ok(())
}

// LiveCli
struct LiveCli {
    model: String,
    allowed_tools: Option<AllowedToolSet>,
    permission_mode: PermissionMode,
    hide_thinking: bool,
    system_prompt: Vec<String>,
    runtime: BuiltRuntime,
    session: SessionHandle,
    prompt_history: Vec<PromptHistoryEntry>,
}
```

```rust
// print_help_to flags section
writeln!(
    out,
    "  --hide-thinking            Hide thinking output in interactive REPL mode"
)?;
```

- [ ] **Step 4: Re-run targeted tests**

Run:
```bash
cd rust
cargo test -p rusty-claude-cli parses_hide_thinking_flag_without_changing_action_shape help_lists_hide_thinking_flag
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd rust
git add crates/rusty-claude-cli/src/main.rs
git commit -m "feat(cli): add hide-thinking flag plumbing"
```

### Task 2: Render thinking live in text mode and remove crab emoji

**Files:**
- Modify: `rust/crates/rusty-claude-cli/src/main.rs` (`run_turn`, `AnthropicRuntimeClient`, `consume_stream`, `push_output_block`, `response_to_events`, tests)
- Test: `rust/crates/rusty-claude-cli/src/main.rs` (`response_to_events_*` tests)

- [ ] **Step 1: Write failing tests for visible thinking rendering**

```rust
#[test]
fn response_to_events_renders_visible_thinking_when_enabled() {
    let mut out = Vec::new();
    let _events = response_to_events(
        MessageResponse {
            id: "msg-thinking-visible".to_string(),
            kind: "message".to_string(),
            model: "claude-opus-4-6".to_string(),
            role: "assistant".to_string(),
            content: vec![
                OutputContentBlock::Thinking {
                    thinking: "step 1".to_string(),
                    signature: Some("sig_123".to_string()),
                },
                OutputContentBlock::Text {
                    text: "Final answer".to_string(),
                },
            ],
            stop_reason: Some("end_turn".to_string()),
            stop_sequence: None,
            usage: Usage::default(),
            request_id: None,
        },
        &mut out,
        true,
    )
    .expect("response conversion should succeed");

    let rendered = String::from_utf8(out).expect("utf8");
    assert!(rendered.contains("▶ Thinking"));
    assert!(rendered.contains("step 1"));
    assert!(!rendered.contains("chars hidden"));
}
```

- [ ] **Step 2: Run targeted tests to confirm failure**

Run:
```bash
cd rust
cargo test -p rusty-claude-cli response_to_events_renders_visible_thinking_when_enabled response_to_events_renders_collapsed_thinking_summary
```

Expected: FAIL (missing `response_to_events` support for `show_thinking`).

- [ ] **Step 3: Implement live-thinking rendering and spinner copy update**

```rust
// run_turn spinner label
spinner.tick(
    "Thinking...",
    TerminalRenderer::new().color_theme(),
    &mut stdout,
)?;
```

```rust
// AnthropicRuntimeClient
struct AnthropicRuntimeClient {
    runtime: tokio::runtime::Runtime,
    client: ApiProviderClient,
    session_id: String,
    model: String,
    enable_tools: bool,
    emit_output: bool,
    show_thinking: bool,
    allowed_tools: Option<AllowedToolSet>,
    tool_registry: GlobalToolRegistry,
    progress_reporter: Option<InternalPromptProgressReporter>,
    reasoning_effort: Option<String>,
}

impl AnthropicRuntimeClient {
    fn set_show_thinking(&mut self, show_thinking: bool) {
        self.show_thinking = show_thinking;
    }
}
```

```rust
// prepare_turn_runtime: configure the runtime before each turn
runtime
    .api_client_mut()
    .set_show_thinking(emit_output && !self.hide_thinking);
```

```rust
fn render_thinking_delta(
    out: &mut (impl Write + ?Sized),
    chunk: &str,
    header_written: &mut bool,
) -> Result<(), RuntimeError> {
    if chunk.is_empty() {
        return Ok(());
    }
    if !*header_written {
        write!(out, "\n▶ Thinking\n").map_err(|error| RuntimeError::new(error.to_string()))?;
        *header_written = true;
    }
    write!(out, "{chunk}")
        .and_then(|()| out.flush())
        .map_err(|error| RuntimeError::new(error.to_string()))
}
```

```rust
// ContentBlockDelta handling
ContentBlockDelta::ThinkingDelta { thinking } => {
    if self.show_thinking {
        render_thinking_delta(out, &thinking, &mut block_has_thinking_summary)?;
    } else if !block_has_thinking_summary {
        render_thinking_block_summary(out, None, false)?;
        block_has_thinking_summary = true;
    }
}
```

- [ ] **Step 4: Re-run targeted thinking tests**

Run:
```bash
cd rust
cargo test -p rusty-claude-cli response_to_events_renders_visible_thinking_when_enabled response_to_events_renders_collapsed_thinking_summary
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd rust
git add crates/rusty-claude-cli/src/main.rs
git commit -m "feat(cli): render live thinking in interactive text mode"
```

### Task 3: Add explicit 10-second waiting notice

**Files:**
- Modify: `rust/crates/rusty-claude-cli/src/main.rs` (`consume_stream`, new wait-notice helpers + tests)
- Test: `rust/crates/rusty-claude-cli/src/main.rs`

- [ ] **Step 1: Write failing tests for wait-notice threshold logic**

```rust
#[test]
fn wait_notice_threshold_triggers_at_ten_seconds() {
    assert!(!should_emit_wait_notice(Duration::from_secs(9), false, false));
    assert!(should_emit_wait_notice(Duration::from_secs(10), false, false));
    assert!(!should_emit_wait_notice(Duration::from_secs(10), true, false));
    assert!(!should_emit_wait_notice(Duration::from_secs(10), false, true));
}

#[test]
fn render_waiting_notice_copy_is_stable() {
    let mut out = Vec::new();
    render_waiting_for_model_notice(&mut out).expect("notice should render");
    let rendered = String::from_utf8(out).expect("utf8");
    assert!(rendered.contains("Aún esperando respuesta del modelo..."));
}
```

- [ ] **Step 2: Run targeted tests to confirm failure**

Run:
```bash
cd rust
cargo test -p rusty-claude-cli wait_notice_threshold_triggers_at_ten_seconds render_waiting_notice_copy_is_stable
```

Expected: FAIL (helpers not implemented).

- [ ] **Step 3: Implement 10s waiting notice in streaming loop**

```rust
const FIRST_VISIBLE_OUTPUT_TIMEOUT: Duration = Duration::from_secs(10);
const STREAM_POLL_INTERVAL: Duration = Duration::from_millis(250);

fn should_emit_wait_notice(
    elapsed: Duration,
    saw_visible_output: bool,
    notice_emitted: bool,
) -> bool {
    !saw_visible_output && !notice_emitted && elapsed >= FIRST_VISIBLE_OUTPUT_TIMEOUT
}

fn render_waiting_for_model_notice(out: &mut (impl Write + ?Sized)) -> Result<(), RuntimeError> {
    write!(out, "\nAún esperando respuesta del modelo...\n")
        .and_then(|()| out.flush())
        .map_err(|error| RuntimeError::new(error.to_string()))
}
```

```rust
// consume_stream loop state
let waiting_started = Instant::now();
let mut wait_notice_emitted = false;
let mut saw_visible_output = false;

// in loop, for normal (non-post-tool-timeout) reads:
let next = if !apply_stall_timeout && self.emit_output && !saw_visible_output {
    match tokio::time::timeout(STREAM_POLL_INTERVAL, stream.next_event()).await {
        Ok(inner) => inner.map_err(|error| RuntimeError::new(format_user_visible_api_error(&self.session_id, &error)))?,
        Err(_elapsed) => {
            if should_emit_wait_notice(waiting_started.elapsed(), saw_visible_output, wait_notice_emitted) {
                render_waiting_for_model_notice(out)?;
                wait_notice_emitted = true;
            }
            continue;
        }
    }
} else {
    stream.next_event().await.map_err(|error| {
        RuntimeError::new(format_user_visible_api_error(&self.session_id, &error))
    })?
};
```

Then set `saw_visible_output = true` whenever text/thinking/tool call output is actually written.

- [ ] **Step 4: Re-run targeted wait-notice tests**

Run:
```bash
cd rust
cargo test -p rusty-claude-cli wait_notice_threshold_triggers_at_ten_seconds render_waiting_notice_copy_is_stable
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd rust
git add crates/rusty-claude-cli/src/main.rs
git commit -m "feat(cli): show 10s waiting notice for delayed model output"
```

### Task 4: Add REPL divider and `› Tu mensaje` input marker

**Files:**
- Modify: `rust/crates/rusty-claude-cli/src/main.rs` (`run_repl`, new boundary helper + tests)
- Test: `rust/crates/rusty-claude-cli/src/main.rs`

- [ ] **Step 1: Write failing tests for prompt boundary formatting**

```rust
#[test]
fn repl_input_boundary_contains_divider_and_prompt_marker() {
    let boundary = format_repl_input_boundary();
    assert!(boundary.contains('─'));
    assert!(boundary.ends_with('\n'));
}

#[test]
fn repl_prompt_label_is_human_readable() {
    assert_eq!(REPL_PROMPT_LABEL, "› Tu mensaje ");
}
```

- [ ] **Step 2: Run targeted tests to confirm failure**

Run:
```bash
cd rust
cargo test -p rusty-claude-cli repl_input_boundary_contains_divider_and_prompt_marker repl_prompt_label_is_human_readable
```

Expected: FAIL (helper/constant missing).

- [ ] **Step 3: Implement boundary rendering in REPL loop**

```rust
const REPL_PROMPT_LABEL: &str = "› Tu mensaje ";

fn format_repl_input_boundary() -> String {
    format!("\n{}\n", "─".repeat(56))
}
```

```rust
// run_repl
let mut editor =
    input::LineEditor::new(REPL_PROMPT_LABEL, cli.repl_completion_candidates().unwrap_or_default());

loop {
    editor.set_completions(cli.repl_completion_candidates().unwrap_or_default());
    print!("{}", format_repl_input_boundary());
    io::stdout().flush()?;
    let _outcome = editor.read_line()?;
}
```

- [ ] **Step 4: Re-run targeted boundary tests**

Run:
```bash
cd rust
cargo test -p rusty-claude-cli repl_input_boundary_contains_divider_and_prompt_marker repl_prompt_label_is_human_readable
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd rust
git add crates/rusty-claude-cli/src/main.rs
git commit -m "feat(repl): add input divider and friendly prompt label"
```

### Task 5: Docs + regression sweep

**Files:**
- Modify: `USAGE.md`
- Validate: `rust/crates/rusty-claude-cli/src/main.rs`, `rust/crates/rusty-claude-cli/tests/*.rs`

- [ ] **Step 1: Update usage docs with thinking visibility behavior**

```md
### Interactive output behavior

- In interactive REPL mode, AGCLI shows live thinking by default.
- Use `--hide-thinking` to collapse thinking output in REPL mode.
- JSON (`--output-format json`) and compact (`--compact`) outputs stay machine-friendly.
```

- [ ] **Step 2: Run focused regression tests for the changed surface**

Run:
```bash
cd rust
cargo test -p rusty-claude-cli \
  parses_hide_thinking_flag_without_changing_action_shape \
  help_lists_hide_thinking_flag \
  response_to_events_renders_visible_thinking_when_enabled \
  response_to_events_renders_collapsed_thinking_summary \
  wait_notice_threshold_triggers_at_ten_seconds \
  repl_input_boundary_contains_divider_and_prompt_marker
```

Expected: PASS.

- [ ] **Step 3: Run full crate tests**

Run:
```bash
cd rust
cargo test -p rusty-claude-cli
```

Expected: PASS.

- [ ] **Step 4: Commit docs + final verification changes**

```bash
cd rust
git add ../USAGE.md crates/rusty-claude-cli/src/main.rs
git commit -m "docs(cli): document interactive thinking and hide-thinking flag"
```

## Dependency order

1. Task 1 must land first (flag plumbing and state propagation).
2. Task 2 depends on Task 1 (`hide_thinking` state available in `LiveCli`/runtime wiring).
3. Task 3 depends on Task 2 (uses updated stream visibility state).
4. Task 4 is independent of Tasks 2–3, but should be merged after Task 1 to minimize conflicts in `main.rs`.
5. Task 5 runs last.

## Self-review checklist (completed)

- **Spec coverage:** Thinking visible by default, no crab emoji, 10s waiting notice, divider + `› Tu mensaje`, `--hide-thinking`, and json/compact invariance are all mapped to explicit tasks.
- **Placeholder scan:** No TBD/TODO placeholders remain.
- **Type consistency:** Plan keeps changes inside existing `main.rs` flow and avoids broad enum-shape churn where not required.
