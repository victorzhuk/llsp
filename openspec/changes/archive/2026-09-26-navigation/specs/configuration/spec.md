# Spec Delta

## ADDED Requirements

### Requirement: Workspace symbol limit
`workspace.max_symbols` (default 256) SHALL cap the number of workspace symbol results.

#### Scenario: Cap
- **WHEN** `workspace.max_symbols = 2` and the query matches five definitions
- **THEN** two results are returned
