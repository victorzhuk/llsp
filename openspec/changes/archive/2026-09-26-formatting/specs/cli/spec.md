# Spec Delta

## ADDED Requirements

### Requirement: Format command
`llsp format <paths>` SHALL re-indent files in place and list changed files; with `--check`
it SHALL write nothing, list files that would change, and exit 1 if any would.

#### Scenario: Check mode
- **WHEN** `llsp format --check .` runs on a badly indented file
- **THEN** the file is listed, left unchanged, and the exit code is 1
