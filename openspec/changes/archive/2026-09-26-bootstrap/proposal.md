# Proposal

## Why

`llsp` starts from an empty repository. Every later change needs a buildable crate,
one place to run checks with resource limits, and CI that rejects regressions.

## What Changes

- Cargo crate `llsp` (lib + bin), edition 2024, `unsafe_code` forbidden.
- `Taskfile.yml` with `build`, `test`, `lint`, `bench`, `deny`, `spec` tasks; test runs carry a
  timeout and a thread cap.
- GitHub Actions workflow: fmt, clippy (`-D warnings`), tests, bench compile, cargo-deny,
  `openspec validate --all --strict`.
- `deny.toml` (advisories, licenses, sources), Apache-2.0 `LICENSE`, `README.md`, `.gitignore`.

## Capabilities

### New Capabilities

None. Tooling only; no observable server behavior.

### Modified Capabilities

None.

## Impact

New repository layout. Adds dependencies listed in `Cargo.toml`.
