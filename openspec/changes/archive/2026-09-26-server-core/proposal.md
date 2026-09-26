# Proposal

## Why

With a reader in place, the server needs a protocol core: lifecycle, document sync with
correct position encoding, one layered configuration model shared by LSP and CLI, and a
command line that also works for batch use in CI.

## What Changes

- LSP over stdio (default) or localhost TCP (opt-in), built on a synchronous main loop.
- Lifecycle per LSP 3.17: initialize, initialized, shutdown, exit; unknown requests answer
  `MethodNotFound`; position encoding negotiated (UTF-8 preferred, UTF-16 fallback).
- Full and incremental document sync; dialect detection per document.
- Layered configuration: built-in defaults < user file < project `.llsp.toml` < `LLSP_*` env <
  CLI `--set` < `initializationOptions` < `workspace/didChangeConfiguration`.
- CLI: `serve` (default), `check` (syntax diagnostics for files, text or JSON output),
  `config` (print effective configuration), `dialects` (list dialects).
- Logging to stderr or a file, off the protocol stream; source text never logged below trace.

## Capabilities

### New Capabilities

- `lsp-lifecycle`: protocol lifecycle, transport, error codes and encoding negotiation.
- `document-sync`: open/change/close handling and dialect detection.
- `configuration`: layered settings from files, environment, CLI and client.
- `cli`: command-line interface for serving and batch use.

### Modified Capabilities

None.

## Impact

New modules `src/server.rs`, `src/document.rs`, `src/config.rs`, `src/main.rs`; tests in `tests/`.
