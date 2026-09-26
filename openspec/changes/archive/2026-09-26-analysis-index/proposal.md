# Proposal

## Why

Navigation, completion, rename and lints all need to know what each symbol means: which
forms define names, which bindings are local, and where every name is used across the
workspace. This must work statically, for every dialect, from the data in dialect files.

## What Changes

- Per-file analysis: definitions (name, kind, parameters, docstring, namespace), local scopes
  and binders with shadowing, symbol occurrences resolved to a local binder or a global name,
  namespace aliases, indentation hints declared in code.
- Workspace index: background scan of workspace roots (gitignore-aware, excludes, file count
  and size limits, no symlink escape), live updates from open documents and watched-file
  events.
- Two new binding shapes, `clauses` and parameter search `list`, needed by existing dialect data.

## Capabilities

### New Capabilities

- `analysis`: static meaning of symbols in a single file.
- `workspace-index`: cross-file knowledge of definitions and references.

### Modified Capabilities

- `dialects`: binding shape `clauses` and parameter search `list` added.

## Impact

New `src/analysis.rs`, `src/workspace.rs`; server gains an event channel and watcher registration.
