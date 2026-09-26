# Tasks

## 1. Crate

- [x] 1.1 Create `Cargo.toml`, `src/lib.rs`, `src/main.rs`; verify `cargo build` succeeds
- [x] 1.2 Add `LICENSE` (Apache-2.0), `README.md`, `.gitignore`; verify files present

## 2. Tooling

- [x] 2.1 Add `Taskfile.yml` with limited test runs; verify `task --list` shows build/test/lint/bench/deny/spec
- [x] 2.2 Add `deny.toml`; verify it parses (`cargo deny check` in CI)
- [x] 2.3 Add `.github/workflows/ci.yml`; verify workflow YAML parses and jobs mirror Taskfile tasks
