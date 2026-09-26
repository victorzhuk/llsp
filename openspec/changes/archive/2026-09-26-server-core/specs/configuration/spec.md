# Spec Delta

## Purpose

Let users tune every server behavior from files, environment, command line or the editor,
with one precedence order and no hard-coded policy.

## ADDED Requirements

### Requirement: Layer precedence
Settings SHALL merge in this order, later winning: built-in defaults, user file
(`$XDG_CONFIG_HOME/llsp/config.toml` or `--config`/`LLSP_CONFIG` path), project file
`.llsp.toml` at the workspace root, `LLSP_*` environment variables, CLI `--set key=value`,
`initializationOptions`, then `workspace/didChangeConfiguration` settings. Tables merge
recursively; other values replace.

#### Scenario: Env over project file
- **WHEN** `.llsp.toml` sets `format.body_indent = 2` and `LLSP_FORMAT__BODY_INDENT=4` is set
- **THEN** the effective value is 4

#### Scenario: Client over CLI
- **WHEN** `--set format.body_indent=3` is given and initializationOptions set `{"format": {"body_indent": 5}}`
- **THEN** the effective value is 5

### Requirement: Environment mapping
Every setting SHALL be settable through `LLSP_<SECTION>__<KEY>` (double underscore for
nesting, case-insensitive), with values parsed as TOML literals and falling back to strings.
`LLSP_CONFIG` and `LLSP_LOG` are reserved.

#### Scenario: Boolean from env
- **WHEN** `LLSP_WORKSPACE__INDEX=false` is set
- **THEN** workspace indexing is disabled

### Requirement: Validation
Unknown keys and wrong value types SHALL be rejected with an error naming the key; at
startup the server fails fast, and on client-sent settings it logs the error, shows a
warning message, and keeps the previous configuration.

#### Scenario: Unknown key from client
- **WHEN** didChangeConfiguration sends `{"fromat": {}}`
- **THEN** the previous configuration stays active and a warning mentions `fromat`

### Requirement: Dialect settings
The `dialects` table SHALL override or extend dialect definitions, and `files.associations`
SHALL map globs to dialect names; `files.default_dialect` names the fallback.

#### Scenario: Unknown dialect in association
- **WHEN** `files.associations` maps `*.x` to `nope`
- **THEN** validation fails naming `nope`
