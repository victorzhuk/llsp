# Proposal

## Why

Every feature (navigation, diagnostics, formatting, highlighting) needs a fast, fault-tolerant,
lossless view of Lisp source across dialects whose reader syntax differs in small ways.
One reader driven by per-dialect data avoids seven grammars and keeps behavior uniform.

## What Changes

- Lossless lexer and error-tolerant parser producing a compact syntax tree for any dialect.
- Dialect definitions as data (TOML), embedded for Common Lisp, Clojure, Scheme, Racket,
  Emacs Lisp, Fennel and Janet; user-extendable through `extends`.
- The reader never evaluates code (`#.`, `#=` and reader conditionals are syntax only).
- Property tests (round-trip, never panics) and parse throughput benchmarks.

## Capabilities

### New Capabilities

- `syntax-reader`: lossless, error-tolerant reading of Lisp source into a syntax tree.
- `dialects`: data-defined dialect reader rules and language knowledge, extendable by users.

### Modified Capabilities

None.

## Impact

New modules `src/syntax/`, `src/dialect.rs`, data files `dialects/*.toml`, bench `benches/parse.rs`.
