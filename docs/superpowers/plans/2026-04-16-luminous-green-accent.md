# Luminous Green Accent Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply a luminous green accent (`#39FF14`) to the approved CLI accent surfaces (`inline_code`, `spinner_done`) without changing the rest of the color palette.

**Architecture:** Keep all behavior in the existing renderer pipeline by updating `ColorTheme::default()` in `render.rs`. Add a focused renderer test first to lock expected ANSI RGB output, then implement the minimal theme change to satisfy the test.

**Tech Stack:** Rust, crossterm, cargo test

---

## File Structure

- Modify: `rust/crates/rusty-claude-cli/src/render.rs`
  - Owns `ColorTheme::default()` and renderer tests.
  - Will receive both the failing test and the minimal implementation change.
- Verify: `rust/crates/rusty-claude-cli/tests/*` (no file edits expected)
  - Run package tests to ensure no regressions.

### Task 1: Lock expected luminous accent behavior with a failing test

**Files:**
- Modify: `rust/crates/rusty-claude-cli/src/render.rs` (test module near existing renderer tests)
- Test: `rust/crates/rusty-claude-cli/src/render.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn renders_inline_code_with_luminous_green_accent() {
    let terminal_renderer = TerminalRenderer::new();
    let markdown_output = terminal_renderer.render_markdown("`code`");

    assert!(
        markdown_output.contains("\u{1b}[38;2;57;255;20m"),
        "inline code should use luminous accent RGB ANSI sequence"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p rusty-claude-cli renders_inline_code_with_luminous_green_accent -- --exact`
Expected: FAIL because current output uses default green (not RGB `57;255;20`).

- [ ] **Step 3: Commit test-only red state**

```bash
git add rust/crates/rusty-claude-cli/src/render.rs
git commit -m "test: lock luminous green inline code expectation"
```

### Task 2: Implement reusable luminous accent in theme defaults

**Files:**
- Modify: `rust/crates/rusty-claude-cli/src/render.rs`
- Test: `rust/crates/rusty-claude-cli/src/render.rs`

- [ ] **Step 1: Write minimal implementation**

```rust
impl Default for ColorTheme {
    fn default() -> Self {
        let luminous_green = Color::Rgb {
            r: 57,
            g: 255,
            b: 20,
        };

        Self {
            heading: Color::Cyan,
            emphasis: Color::Magenta,
            strong: Color::Yellow,
            inline_code: luminous_green,
            link: Color::Blue,
            quote: Color::DarkGrey,
            table_border: Color::DarkCyan,
            code_block_border: Color::DarkGrey,
            spinner_active: Color::Blue,
            spinner_done: luminous_green,
            spinner_failed: Color::Red,
        }
    }
}
```

- [ ] **Step 2: Run targeted tests to verify pass**

Run: `cargo test -p rusty-claude-cli renders_inline_code_with_luminous_green_accent -- --exact`
Expected: PASS.

- [ ] **Step 3: Run package tests for renderer safety**

Run: `cargo test -p rusty-claude-cli render`
Expected: PASS for render-related unit/integration coverage.

- [ ] **Step 4: Commit implementation**

```bash
git add rust/crates/rusty-claude-cli/src/render.rs
git commit -m "feat: apply luminous green accent to renderer theme defaults"
```

### Task 3: Final regression check

**Files:**
- Verify only (no edits): `rust/crates/rusty-claude-cli/src/render.rs`, `rust/crates/rusty-claude-cli/tests/*`

- [ ] **Step 1: Run full crate tests**

Run: `cargo test -p rusty-claude-cli`
Expected: PASS.

- [ ] **Step 2: Inspect working tree for scope control**

Run: `git --no-pager status --short`
Expected: only intended files changed for this feature.

- [ ] **Step 3: Commit final cleanup if needed**

```bash
git add -A
git commit -m "chore: finalize luminous green accent change"
```
