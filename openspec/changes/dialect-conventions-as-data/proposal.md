# Proposal

## Why

The dialects capability promises dialect behavior "configurable without code changes and
new dialects can be added", but several per-dialect conventions are hardcoded in Rust, so a
user dialect with different conventions needs a Rust change:

- Rest-parameter markers `&rest`, `&body`, `&`, `.`, `&more` and the `&key` marker in
  signature help (`src/features/assist.rs:457-472`) — a cross-dialect union no single
  dialect can vary.
- Indent declaration syntax `(declare (indent N))` and `:style/indent N`
  (`src/analysis.rs:404-439`).
- Regexp string prefixes `#`, `#rx`, `#px` in semantic highlighting
  (`src/features/structure.rs:231-236`) — again a cross-dialect union, applied regardless
  of dialect.
- The cell-position shapes — `#'` prefix, `function` form, `cond` clause tests, reader
  vectors `#(...)` — are encoded twice: in the analysis walker
  (`src/analysis.rs:694-730`) and again for the completion gap (`src/features/assist.rs:338-387`).

## What Changes

- Dialect fields (names finalized in tasks) declaring rest markers, indent declaration
  forms and regexp string prefixes, with the current stock values as defaults; analysis,
  signature help and semantic tokens read them from the dialect.
- The cell-position predicates (`#'`, `function`, cond clause test, reader vector) are
  extracted into shared tree-level functions used by both the analysis walker and the
  completion gap logic. They deliberately stay code: they encode the cell model itself, not
  per-dialect vocabulary, and no dialect today needs to vary them.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `dialects`: call and declaration conventions become dialect data.
- `structure`: regexp string tokens come from the dialect's declared prefixes.

## Impact

`src/dialect.rs` (+ all nine `dialects/*.toml` gain nothing — defaults carry current
values), `src/analysis.rs`, `src/features/assist.rs`, `src/features/structure.rs`. Stock
dialect behavior is unchanged byte for byte; only user dialects can now express different
conventions.
