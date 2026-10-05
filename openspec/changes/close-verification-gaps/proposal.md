# Proposal

## Why

A requirement-by-requirement trace of the 15 specs against the test suite (154 tests,
measured union line coverage ≥ 93%) found two spec promises with zero tests, one systemic
blind spot, and a cluster of untested hostile-input paths:

- Zero tests: the security spec's "logs SHALL NOT contain document text" promise, and the
  navigation spec's document-symbol nesting ("definitions nested inside another
  definition's form are its children") — the only untested logic in
  `src/features/symbols.rs:20-48`.
- CRLF: four `\r` branches across sync (`src/document.rs:146-147`), format
  (`src/format.rs:100-110`), semantic-token encoding (`src/features/structure.rs:378-385`)
  and the lexer, with not one test feeding a carriage return — the fuzz strategies
  structurally cannot generate one.
- Hostile state transitions: overlapping and inverted incremental edits (the
  `end.max(start)` guard at `src/document.rs:101`), invalid startup configs (garbage user
  or project TOML, conflicting env vars), and the robustness fuzz never exercising
  `didClose`, watched-file events or config reload.
- Unasserted spec details: hover's `file` field and multi-definition join, completion's
  "each label appears once" and binder self-exclusion, references on an ambiguous
  unqualified global.
- Dead code with zero call sites: `Analysis::binder_index_at` (`src/analysis.rs:105`),
  `Tree::token_index_at` (`src/syntax/mod.rs:258`), `Document::lines`,
  `dialect::yes`/`dialect::parens` serde defaults.

## What Changes

- New and extended tests only, prioritized P0 (spec promise with zero tests), P1 (risky
  logic partially covered), P2 (nice-to-have). Dead public helpers are deleted rather than
  tested. No behavior change.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. Verification of existing requirements.

## Impact

`tests/log_hygiene.rs` (new), `tests/navigation.rs`, `tests/assist.rs`, `tests/robustness.rs`,
`tests/lifecycle.rs`, `tests/cli.rs`, `tests/sync.rs`, inline test modules in
`src/document.rs` and `src/format.rs`, `src/syntax/tests.rs` fuzz strategies; deletions in
`src/analysis.rs`, `src/syntax/mod.rs`, `src/document.rs`, `src/dialect.rs`. No production
behavior change.
