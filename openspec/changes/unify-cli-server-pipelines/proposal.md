# Proposal

## Why

The CLI and the server hand-copy the same policy in two places, and one copy has already
drifted into spec-divergent behavior:

- Formatting hints: `textDocument/formatting` resolves `(declare (indent N))` hints
  workspace-wide (`src/features/formatting.rs:33-38`); `llsp format` resolves them from the
  file being formatted only (`src/main.rs:312-318`). A macro declared in `a.el` affects the
  editor's formatting of `b.el` but not `llsp format b.el` — the same repo formats
  differently in the two front ends, and the formatting spec's declared-indent scenario
  says the hinting definition is "in the workspace".
- The lint `is_defined` policy (same dialect + function cell over `defs_named`) is written
  twice (`src/main.rs:236-240`, `src/server.rs:624-629`); the cli spec requires `llsp check`
  to report the same lints as the server, but nothing enforces it structurally.
- File budget: the server's scan applies one `workspace.max_files` budget across all roots;
  `collect_files` grants each path argument its own budget (`src/main.rs:349-370`), so
  `llsp check dir1 dir2` can process `2 × max_files` files.

## What Changes

- One shared definition-lookup helper on `Index` (dialect + cell filter), used by the
  server lint path, the CLI check path, and the feature handlers that re-implement the
  filter today; the shared `is_defined` closures call it.
- One shared indent-hints helper resolving hints across the workspace; `llsp format` builds
  an index over all input files (as `llsp check` already does) and uses it.
- `collect_files` applies a single `workspace.max_files` budget across all path arguments
  and reports truncation once.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `cli`: format resolves workspace hints; one file budget across path arguments.

## Impact

`src/main.rs`, `src/server.rs`, `src/workspace.rs`, `src/features/{mod,assist,navigation,structure}.rs`.
User-visible changes: `llsp format` output matches the editor for workspaces with
cross-file indent hints; multi-directory CLI runs stop at one budget. No LSP behavior
change.
