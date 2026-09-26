# Design

## Context

See proposal.md. The reader feeds every later feature, so it must be fast, lossless and total.

## Goals / Non-Goals

Goals: one reader for all dialects; linear time; no recursion; stable byte ranges.
Non-goals: incremental reparsing (full reparse is cheap enough, measured by benches),
reader-macro execution, numeric/semantic interpretation of atoms beyond classification.

## Decisions

- **Own arena tree instead of rowan.** Tokens live in one `Vec<Token>` (kind, start, len);
  forms live in one `Vec<Node>` (kind, byte range, parent, children slice into a shared
  `Vec<NodeId>`). Children are collected on a stack and copied as one contiguous slice when a
  list closes. Trivia stay in the token vector only, so analysis walks forms without skipping
  comments, and losslessness is a property of the token vector. rowan's green/red trees add a
  dependency (latest release is an alpha) and API surface we do not need; parent links and
  child slices cover every query we have (node at offset, ancestors, siblings).
- **Lexer driven by `ReaderRules`.** Longest-match on configured prefixes, then generic rules:
  - `#` followed by constituents is an atom (`#t`, `#x1F`, `#:foo`), or a prefix when an opener
    or `"` follows immediately (`#hash(`, `#rx"`, `#p"`, `#2A(`).
  - `#` directly before an opener or `"` is a prefix (`#(`, `#{`, `#"`).
  - `#N=` is a prefix, `#N#` an atom.
  - A character literal consumes one char after its prefix (escaped once), then constituents.
- **Prefix arity.** Each prefix binds N following forms (`#+` binds 2, `^` binds 2, default 1).
  The parser keeps a pending-prefix frame on the same explicit stack as lists.
- **Datum comments** wrap the next form in a `DatumComment` node; analysis skips it.
- **Error cap.** At most 100 syntax errors per file; the rest are dropped.
- **Dialect data in TOML**, embedded with `include_str!`, merged as `toml::Table` values
  (deep merge, arrays replace) before deserializing into typed structs. `extends` chains
  are resolved by merging parent first; cycles are an error.

## Risks / Trade-offs

- [Heuristic `#name` handling misreads an exotic reader macro] → prefixes and char rules are
  configurable per dialect; misreads stay local to one form.
- [Tagged literals with whitespace (`#inst "..."`) read as two forms] → harmless for analysis.
