# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
