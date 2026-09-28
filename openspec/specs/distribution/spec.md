# distribution Specification

## Purpose
Ship prebuilt llsp binaries and a one-line installer.

## Requirements

### Requirement: Release artifacts
Pushing a `vX.Y.Z` tag SHALL publish a GitHub release only when the tag matches the
`Cargo.toml` version and `CHANGELOG.md` has a section for it. The release SHALL contain
`llsp-<target>.tar.gz` for x86_64 and aarch64 Linux (statically linked, musl) and x86_64 and
aarch64 macOS, `llsp-x86_64-pc-windows-msvc.zip`, a `SHA256SUMS` file covering every archive,
and `install.sh`; its notes are the changelog section.

#### Scenario: Version mismatch
- **WHEN** tag `v0.2.0` is pushed while `Cargo.toml` says `0.1.0`
- **THEN** the workflow fails before building and no release is created

### Requirement: Install script
`install.sh` SHALL detect the OS and CPU, download the matching archive and `SHA256SUMS`,
refuse to install when the checksum does not match, and install `llsp` into
`LLSP_INSTALL_DIR` (default `~/.local/bin`), noting when that directory is not in `PATH`.
`LLSP_VERSION` selects a release tag (default latest) and `LLSP_BASE_URL` a download
location. On Apple silicon it SHALL install the arm64 build even from an x86_64 shell.

#### Scenario: Checksum mismatch
- **WHEN** the downloaded archive does not match its `SHA256SUMS` entry
- **THEN** the script exits with an error and installs nothing

#### Scenario: Default install
- **WHEN** `curl -fsSL https://github.com/victorzhuk/llsp/releases/latest/download/install.sh | sh` runs on x86_64 Linux
- **THEN** the static x86_64 build is installed as `~/.local/bin/llsp`
