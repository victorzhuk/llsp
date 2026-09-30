# Proposal

## Why

go-lispico ships a Clojure dialect and a Common Lisp dialect with their own reader, special
forms, builtins and, for CL, a separate function cell. The stock `clojure` and `common-lisp`
dialects accept syntax go-lispico rejects (`#_`, `\c`, `#{}`, `[]` in CL), offer builtins it
does not have, and resolve a CL call head to a local variable the runtime would never call.
zed-lisp is moving its Lispico modes to llsp, and this is the first step.

## What Changes

- Two built-in dialects, `lispico-clojure` and `lispico-cl`, derived from go-lispico sources:
  kernel and dialect special forms, stdlib builtins and bootstrap macros, CL vocabulary and
  adapters. They claim no file extensions; files select them by language ID or
  `files.associations`.
- Dialect flag `function_cells`: in a Lisp-2 dialect a call head and the argument of
  `function`/`#'` resolve only to function bindings, and every other position resolves only to
  value bindings. Definition forms declare which cell they bind.
- Reader rule `invalid`: token openers the dialect rejects (`[` `]` `{` `}` for `lispico-cl`,
  bare `#` for `lispico-clojure`) are read as one-byte atoms carrying an `invalid-syntax` error
  over their own range, instead of the hard read error go-lispico raises.

## Capabilities

### Modified Capabilities

- `dialects`: two new built-in dialects; `function_cells` and per-definition cell.
- `syntax-reader`: dialect-rejected openers are syntax errors.
- `analysis`: cell-aware resolution in Lisp-2 dialects.
- `diagnostics`: `unresolved-call` honors cells.

## Impact

`dialects/lispico-*.toml`, `src/dialect.rs`, `src/syntax` reader, `src/analysis.rs`,
`src/workspace.rs` (cell-aware lookup of definitions in other files of the same dialect),
`src/diagnostics.rs`, tests, README dialect list, CHANGELOG. Existing dialects are unchanged.
Project contexts, library selection and host catalogs come in later changes.
