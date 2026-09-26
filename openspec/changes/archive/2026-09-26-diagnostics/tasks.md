# Tasks

## 1. Lints

- [x] 1.1 Add `diagnostics` settings with `Level` enum; verify config unit tests incl. invalid severity
- [x] 1.2 Implement unused-binding, duplicate-definition, unresolved-call; verify unit tests per scenario

## 2. Publishing

- [x] 2.1 Debounced publishing and re-publish after indexing; verify integration tests
- [x] 2.2 `enable = false` and per-lint `off`; verify integration tests

## 3. CLI

- [x] 3.1 `check` builds an index of checked files and runs lints; verify CLI test
- [x] 3.2 Document lints and settings in README; verify keys match `llsp config`
