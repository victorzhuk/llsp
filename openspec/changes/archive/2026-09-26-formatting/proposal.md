# Proposal

## Why

Lisp style is mostly indentation. A formatter that only re-indents, driven by per-dialect
indent specs and hints declared in code, is predictable, safe (it never moves tokens between
lines) and works the same across dialects, in the editor and in CI.

## What Changes

- `textDocument/formatting` and `textDocument/rangeFormatting`: re-indent lines, trim
  trailing whitespace, leave string and block-comment interiors untouched.
- Rules: data brackets `[]`/`{}` indent one past the opener; a form with an indent spec N
  indents its first N arguments by `format.distinguished_indent` and the rest by
  `format.body_indent`; other calls align with the first argument when it shares the head
  line, else one past the opener. Specs come from `(declare (indent N))`/`:style/indent`
  hints, then the dialect `indent` table, then `indent_prefixes`.
- CLI `llsp format <paths>` rewrites files; `--check` reports files that would change.
- Common Lisp drops the Emacs Lisp style `if` spec; Clojure gains `if`/`if-not` specs.

## Capabilities

### New Capabilities

- `formatting`: indentation-only formatting.

### Modified Capabilities

- `cli`: `format` command.

## Impact

New `src/format.rs` (shared by server and CLI), `src/features/` handler, `src/main.rs`.
