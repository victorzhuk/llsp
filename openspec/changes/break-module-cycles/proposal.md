# Proposal

## Why

The module dependency graph has three genuine cycles and one misplaced policy, all
verified from `use crate::` imports:

- `server ⇄ features`: every feature file is an `impl Server` block mutating `pub(crate)`
  state (`src/features/mod.rs:15`, `src/server.rs:32-36`); `Server` fuses transport,
  lifecycle, state and scheduling, and no feature logic is unit-testable without a full LSP
  loop.
- `workspace ⇄ features`: the indexing core calls `crate::features::score_lowercase`
  (`src/workspace.rs:90`) while features import `workspace::{FileSummary, Ref}` — dependency
  direction inversion that blocks any crate split.
- `config ⇄ dialect`: `config.rs` loads `Dialects` while `dialect.rs` calls
  `config::merge_tables` (`src/config.rs:7`, `src/dialect.rs:8`) — the dialect-data module
  depends on the configuration layer for a generic TOML utility.
- `Settings::detect` (dialect detection policy) lives in `document.rs:225-264` with four
  call sites elsewhere; LSP error codes (`HandlerResult`) are baked into feature policy.

## What Changes

- Extract the fuzzy-score functions to `src/search.rs`; `workspace` and `features` both
  depend on it.
- Extract `merge_tables`/`json_to_table` to `src/tables.rs`; `config` and `dialect` both
  depend on it.
- Extract session state (`docs`, `index`, `settings`, `enc`, `roots`) into a `Session` type;
  features become free functions over `&Session`/`&mut Session`, `Server` keeps transport
  and dispatch only.
- Move `impl Settings { detect }` to `config.rs`.
- Introduce a `FeatureError` enum; LSP codes are mapped at the dispatch site in `server.rs`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. Internal structure only; observable behavior is unchanged and pinned by the existing
suite.

## Impact

`src/{server,features/*,workspace,config,dialect,document,main}.rs`. Enables unit tests for
policy logic without a `Connection`, and any future crate split. Guarded by `task test`,
clippy, and the traceability tests from `close-verification-gaps`.
