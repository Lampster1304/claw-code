# Luminous Green Accent Design

## Problem
The CLI uses the default `Color::Green` for key accent signals (`inline_code` and `spinner_done`).  
We need a more luminous green tone while keeping the rest of the palette unchanged.

## Proposed Approach
Introduce one reusable accent color in the renderer and apply it only to the two approved accent surfaces:

- `inline_code`
- `spinner_done`

Target color: `#39FF14` (`Color::Rgb { r: 57, g: 255, b: 20 }`).

## Architecture and Components
- File: `rust/crates/rusty-claude-cli/src/render.rs`
- Component: `ColorTheme::default()`
- Change:
  - define a local/reusable accent variable with `Color::Rgb { r: 57, g: 255, b: 20 }`
  - assign that accent to `inline_code` and `spinner_done`

No changes to command parsing, runtime state, or configuration schema.

## Data Flow / Behavior
Color rendering stays in the existing pipeline:
`ColorTheme::default()` -> renderer style application -> terminal ANSI output.

Only the selected accent fields emit different ANSI color values.

## Error Handling
No new error paths are introduced.  
Rendering keeps using current crossterm behavior.

## Testing
- Keep existing test suite behavior unchanged.
- If tests assert exact ANSI output for green accents, update expected output to match the RGB accent.
- Otherwise rely on existing renderer and CLI tests.

## Scope Boundaries
In scope:
- Accent color replacement for `inline_code` and `spinner_done`.

Out of scope:
- Full palette redesign.
- New theme/configuration options.
- Slash command behavior changes.
