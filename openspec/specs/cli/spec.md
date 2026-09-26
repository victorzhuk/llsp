# cli Specification

## Purpose
Provide a command line for running the server and for batch checks usable in scripts and CI.

## Requirements

### Requirement: Commands
The binary SHALL provide `serve` (default when no command is given), `check <paths>`,
`config` and `dialects`, plus `--version` and `--help`.

#### Scenario: Default command
- **WHEN** `llsp` runs with no arguments
- **THEN** it serves LSP over stdio

### Requirement: Machine-readable output
`check`, `config` and `dialects` SHALL support `--format text|json`; the default is text when
stdout is a terminal and JSON otherwise.

#### Scenario: Piped check
- **WHEN** `llsp check broken.lisp | cat` runs
- **THEN** stdout is a JSON array of diagnostics with path, line, column, severity, code and message

### Requirement: Exit status
`check` SHALL exit 1 when any error-severity diagnostic is found, 2 on usage or I/O errors,
and 0 otherwise.

#### Scenario: Clean file
- **WHEN** `llsp check ok.clj` runs on valid code
- **THEN** it exits 0 and prints no diagnostics

### Requirement: Check runs lints
`llsp check` SHALL report the same lints as the server, resolving names across all files
being checked, with the same configuration.

#### Scenario: Cross-file call
- **WHEN** `llsp check --set diagnostics.unresolved_call=\"warning\" .` runs on `a.lisp` defining `helper` and `b.lisp` calling it
- **THEN** no `unresolved-call` is reported for `helper`

### Requirement: Format command
`llsp format <paths>` SHALL re-indent files in place and list changed files; with `--check`
it SHALL write nothing, list files that would change, and exit 1 if any would.

#### Scenario: Check mode
- **WHEN** `llsp format --check .` runs on a badly indented file
- **THEN** the file is listed, left unchanged, and the exit code is 1
