# Tasks

## 1. Shared lookup

- [x] 1.1 Add `Index::defs_in(key, dialect, cell)` returning same-dialect, cell-matching
      definitions; replaced the hand-written filter at the resolution sites (`src/main.rs`
      check, `src/server.rs` publish_diagnostics, `session::defined_namespaces`,
      `navigation::global_definitions`, `assist` signature help and hover); the per-file
      declaration filter in `references` and the cell-free kind lookup in
      `structure::classify` keep their own (different) predicates on purpose; verify
      `task test` green and `grep -rn "defs_named" src/` shows no caller repeating the
      dialect/cell predicate

## 2. Format parity

- [x] 2.1 Shared `workspace::indent_hints(index, dialect)` used by both `format_edits`
      (`src/features/formatting.rs`) and `format_files`; `format_files` builds an `Index`
      over all collected files (as `check` does) and reuses it; verify the two-file test
      `format_resolves_workspace_indent_hints`: `llsp format .` produces the same
      `body_indent` indentation `textDocument/formatting` returns (`tests/formatting.rs`
      keeps covering the LSP side)

## 3. File budget

- [x] 3.1 `collect_files` accumulates all directory arguments under one
      `workspace.max_files` budget (explicitly named files are always processed) with one
      truncation warning; `workspace::discover` takes the budget explicitly; verify
      `format_check_applies_one_file_budget` processes exactly `max_files` files across two
      directories and prints one warning
