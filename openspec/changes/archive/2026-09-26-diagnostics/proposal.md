# Proposal

## Why

Syntax errors alone miss common mistakes: unused bindings, a definition accidentally
duplicated, a call to a name that exists nowhere. Each team weighs these differently, so every
lint needs its own configurable severity, and editor and CLI must report the same results.

## What Changes

- Lints `unused-binding` (hint, tagged unnecessary), `duplicate-definition` (warning) and
  `unresolved-call` (off by default), each with a configurable severity or `off`.
- `diagnostics.enable`, `diagnostics.debounce_ms`, `diagnostics.ignore_prefix`,
  `diagnostics.known_symbols` settings.
- Open documents are re-checked after the workspace index loads.
- `llsp check` runs the same lints, resolving names across all checked files.
- Dialect field `data_forms`: heads whose arguments are data (clauses, slot specs, patterns),
  so `unresolved-call` does not treat them as calls.

## Capabilities

### New Capabilities

- `diagnostics`: static lints with configurable severities.

### Modified Capabilities

- `configuration`: diagnostics settings.
- `cli`: `check` reports lints.
- `dialects`: `data_forms` field.

## Impact

`src/diagnostics.rs`, server publishing loop, `src/main.rs` check command.
