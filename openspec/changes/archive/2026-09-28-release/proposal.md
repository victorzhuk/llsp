# Proposal

## Why

llsp could only be installed with `cargo install`, which needs a Rust toolchain and a build.

## What Changes

- A tag-triggered release workflow builds static Linux binaries (x86_64, aarch64), macOS
  binaries (x86_64, arm64) and a Windows binary, and publishes them with SHA-256 checksums,
  the changelog section as notes, and `install.sh`.
- `install.sh` picks the right build, verifies its checksum and installs into `~/.local/bin`.
- Linux builds use mimalloc: musl's allocator made the parallel workspace scan ~20x slower.
- `CHANGELOG.md` records user-visible changes per release.

## Capabilities

### New Capabilities

- `distribution`: release artifacts and the install script.

## Impact

New `.github/workflows/release.yml`, `install.sh`, `CHANGELOG.md`; CI checks the install
script; `mimalloc` for musl targets only.
