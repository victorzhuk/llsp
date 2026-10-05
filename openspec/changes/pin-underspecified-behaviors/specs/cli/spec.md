## MODIFIED Requirements

### Requirement: Machine-readable output
`check`, `format`, `config` and `dialects` SHALL support `--format text|json`; the default
is text when stdout is a terminal and JSON otherwise.

#### Scenario: Piped check
- **WHEN** `llsp check broken.lisp | cat` runs
- **THEN** stdout is a JSON array of diagnostics with path, line, column, severity, code and message

#### Scenario: Piped format check
- **WHEN** `llsp format --check . | cat` runs
- **THEN** stdout is a JSON array of the paths that would change
