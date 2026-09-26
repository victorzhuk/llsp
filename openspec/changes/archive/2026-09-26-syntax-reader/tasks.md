# Tasks

## 1. Dialect data

- [x] 1.1 Define `Dialect`/`ReaderRules` types and TOML loading with `extends` merge; verify unit tests for override and cycle error
- [x] 1.2 Write `dialects/*.toml` for the seven built-in dialects; verify a test loads all seven

## 2. Reader

- [x] 2.1 Implement lexer with per-dialect rules; verify unit tests for comments, strings, chars, prefixes per dialect
- [x] 2.2 Implement iterative parser with errors and error cap; verify tests for each error kind and 100000-deep nesting
- [x] 2.3 Add proptest round-trip and no-panic properties; verify `task test` passes

## 3. Benchmarks

- [x] 3.1 Add `benches/parse.rs` (lex+parse per dialect on generated 1 MB input); verify `cargo bench --no-run` builds
