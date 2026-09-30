# Tasks

## 1. Reader

- [x] 1.1 Add `reader.invalid` and the `invalid-syntax` `ErrorKind`; a declared opener is matched before every other rule for that byte in `next_kind`, is read as a one-byte `Atom` and reports one error over that token, with no `OPEN`/`CLOSE` classification for it; verify reader unit tests (one error per token at `[` and `]` of `(f [x])`, one at `#` in `#{1 2}`, `#'` and `#(` still valid in `lispico-cl`, stock dialects byte-identical) and the round-trip property test

## 2. Dialect data

- [x] 2.1 Add `function_cells` dialect flag and a per-definition `cell`; local binders bind the value cell in every dialect, so the stock `flet` shapes keep one namespace; verify dialect load tests (default off, unknown keys still rejected)
- [x] 2.2 Add `dialects/lispico-clojure.toml` (`reader.invalid = ["#"]`, no `#` prefixes, `sharp_dispatch = false`) and `dialects/lispico-cl.toml` (`reader.invalid = ["[", "]", "{", "}"]`, `#'` prefix, `sharp_dispatch = true`) from go-lispico `12e0990`; make the extension assertions in `loads_all_builtins` (`src/dialect.rs:468-478`) and `dialects_listed` (`tests/cli.rs:117-137`) per-dialect — non-empty extensions required for the seven stock dialects, empty required for the two Lispico ones, so a Lispico file claiming `.clj` or `.lisp` fails; verify `llsp dialects` lists both with no extensions and `task test`
- [x] 2.3 Reader fixtures per dialect: accepted forms from go-lispico `clojure/clojure_test.go`, `cl/cl_test.go` and `internal/goldset/testdata` read without errors; rejected openers report `invalid-syntax`; verify integration tests

## 3. Resolution

- [x] 3.1 Cell-aware binders, definitions and occurrences in Lisp-2 dialects; verify analysis unit tests (call head skips `let` binder; `#'f` and `(function f)` resolve to `defun f`; `def` and `defun` of one name stay separate)
- [x] 3.2 `unresolved-call`, definition, references, hover and completion respect cells, including definitions reached through the workspace index in other files of the same dialect (`src/workspace.rs` `def_keys` / `Index::by_def`); verify integration tests in `tests/navigation.rs`, `tests/diagnostics.rs`, `tests/assist.rs` with a two-file project
- [x] 3.3 Existing dialects unchanged, including the stock `flet` shapes, whose single namespace is untouched because their `function_cells` flag stays off; widen the two proptest dialect arrays to nine — `DIALECTS` and `d in 0usize..7` in `src/syntax/tests.rs:190-198,230,245` and `tests/robustness.rs:8-16,114` — so the Lispico dialects are inside the round-trip and robustness properties, addressed by language ID since they have no extension; verify full `task test`

## 4. Docs

- [x] 4.1 README dialect list and `files.associations` example for Lispico projects; CHANGELOG `[Unreleased]`; verify `task spec`
