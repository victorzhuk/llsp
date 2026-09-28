# Tasks

## 1. Artifacts

- [x] 1.1 Release workflow: tag/version check, target matrix, smoke tests, archives, checksums, notes from CHANGELOG; verify with actionlint
- [x] 1.2 mimalloc as the global allocator on musl; verify workspace scan time against glibc

## 2. Install

- [x] 2.1 `install.sh` with OS/CPU detection, checksum verification and `LLSP_VERSION`, `LLSP_INSTALL_DIR`, `LLSP_BASE_URL`; verify with shellcheck and installs from a local mirror
- [x] 2.2 CI job installing from a stub mirror on Linux and macOS; README install and release instructions
