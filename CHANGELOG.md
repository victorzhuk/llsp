# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1] - 2026-10-07

### Fixed
- `workspace::scan` canonicalizes its roots before the read-time confinement check, so
  library callers on macOS (where `/var` is a symlink into `/private/var`) get a working
  scan instead of an empty index when roots are passed uncanonicalized.

## [0.3.0] - 2026-10-07

### Added
- Dialects can now declare their own signature and highlighting conventions:
  `rest_markers`, `key_markers`, `regexp_string_prefixes` and `indent_declarations`,
  with the stock dialects' current values as defaults.

### Fixed
- Watched-file event bursts and document close no longer block request handling: index
  updates are summarized off the main loop and applied in order.
- Completion no longer clones every workspace match before ranking: candidate data is
  materialized only for the returned items.
- Analyzing definitions with very large parameter lists no longer stalls (quadratic
  pattern dedup and per-scope binder lookup are now hash-based).
- `llsp format` now resolves `(declare (indent N))` hints across all files it processes,
  matching what the editor's formatting produces for the same workspace, instead of
  seeing only the file being formatted.
- `check` and `format` apply one `workspace.max_files` budget across all directory
  arguments (previously one per directory), with a single truncation warning.
- Hostile configuration can no longer abort the server: `extends` chains are depth- and
  count-bounded, numeric settings (`diagnostics.debounce_ms`, `format.body_indent`,
  `format.distinguished_indent`) have documented maxima, and formatter indentation is
  clamped.
- `didChangeConfiguration`, `didChangeWatchedFiles` and `didClose` notifications are
  panic-isolated: a panic in one is logged and the server keeps running.
- Signature labels compute parameter offsets in one pass and cap collected parameters,
  so definitions with pathological parameter lists no longer stall hover and signature
  help.
- Log files keep mode 0600 when they already exist, and a symlinked log path is refused
  instead of being written through.
- Workspace confinement is enforced when a file is read, re-checking the canonicalized
  path against the roots.

## [0.2.1] - 2026-09-30

### Fixed
- The client-reported language ID is retained across configuration reloads, so
  extensionless files keep their dialect (e.g. the Lispico languages) after any
  settings change.
- The installer fails instead of silently succeeding when the installed binary
  cannot execute.

## [0.2.0] - 2026-09-30

### Added
- Two Lispico dialects, `lispico-clojure` and `lispico-cl`, with `function_cells` support
  and `reader.invalid` diagnostics. They claim no file extensions and are selected by
  language ID or `files.associations`.
- Note: semantic token classification stays keyed on the bare name, so a name in value
  position that shares a function definition may still be colored as a function.

## [0.1.4] - 2026-09-29

### Changed
- `LICENSE` names the copyright holder instead of the Apache boilerplate placeholder.

## [0.1.3] - 2026-09-29

### Changed
- Shorter README covering install, usage, configuration and the security model.

## [0.1.2] - 2026-09-28

### Fixed
- The release check fails when the installer download fails instead of passing silently.

## [0.1.0] - 2026-09-28

First release.

### Added
- Language server over stdio, or over TCP on a loopback address with
  `llsp serve --listen`.
- Lossless reader for Common Lisp, Clojure, Scheme, Racket, Emacs Lisp, Fennel and Janet,
  each defined by a TOML dialect file that can be overridden or extended.
- Syntax diagnostics and lints (`unused-binding`, `duplicate-definition`, `unresolved-call`),
  each with its own severity, published after a configurable debounce.
- Workspace index honoring `.gitignore` and `workspace.exclude`, updated from file watcher
  events and open documents.
- Document outline and fuzzy workspace symbol search.
- Go to definition, find references and document highlights for locals and workspace names.
- Rename for locals and workspace names. Workspace renames only touch references that
  resolve to the renamed definition's namespace, through qualifiers, aliases and `:refer`.
- Completion, signature help and hover.
- Semantic highlighting, folding ranges and expand selection.
- Indentation-only formatting of documents and ranges.
- `llsp check`, `llsp format`, `llsp config` and `llsp dialects`, with JSON output when
  stdout is not a terminal.
- Layered configuration: user file, project `.llsp.toml`, environment, command line and
  editor settings.
- Static Linux binaries for x86_64 and aarch64, macOS and Windows builds, and an install
  script.

### Security
- llsp never evaluates code: no reader macros, no `#.`, no subprocesses and no network access
  besides the explicit loopback listener.
- Documents over `files.max_file_size` are kept in sync but not analyzed.
- The project `.llsp.toml` cannot configure logging, and log files are created readable only
  by their owner.

[unreleased]: https://github.com/victorzhuk/llsp/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/victorzhuk/llsp/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/victorzhuk/llsp/compare/v0.1.4...v0.2.0
[0.1.4]: https://github.com/victorzhuk/llsp/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/victorzhuk/llsp/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/victorzhuk/llsp/compare/v0.1.0...v0.1.2
[0.1.0]: https://github.com/victorzhuk/llsp/releases/tag/v0.1.0
