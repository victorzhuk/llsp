# Tasks

## 1. Configuration

- [x] 1.1 Implement `Config` sections (files, workspace, format, log, dialects) with defaults and `deny_unknown_fields`; verify unit test for defaults and unknown-key error
- [x] 1.2 Implement layers: user/project files, env mapping, `--set`, client JSON; verify unit tests for precedence, env parsing and JSON conversion
- [x] 1.3 Validate associations and default dialect against loaded dialects; verify unit test naming the unknown dialect

## 2. Documents

- [x] 2.1 Implement URI/path conversion with percent-decoding; verify unit tests incl. spaces and non-file schemes
- [x] 2.2 Implement `Document` with incremental edits and UTF-8/UTF-16 conversion with clamping; verify unit tests for edits, multibyte text and out-of-range ranges
- [x] 2.3 Implement dialect detection order; verify unit tests for association, languageId, modeline, `#lang`, extension, default

## 3. Server

- [x] 3.1 Main loop with lifecycle, dispatch, MethodNotFound and InvalidParams; verify integration tests over an in-memory connection
- [x] 3.2 Document sync and syntax diagnostics publish/clear; verify integration tests for open/change/close
- [x] 3.3 didChangeConfiguration with validation and fallback; verify integration test for unknown key warning

## 4. CLI

- [x] 4.1 `serve` (stdio, loopback-only `--listen`), `check`, `config`, `dialects` with text/json output and exit codes; verify CLI tests running the built binary
- [x] 4.2 Document configuration and CLI in README; verify examples match `llsp --help`
