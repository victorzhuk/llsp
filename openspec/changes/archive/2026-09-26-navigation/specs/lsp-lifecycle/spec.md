# Spec Delta

## ADDED Requirements

### Requirement: Invalid parameters
The server SHALL answer requests whose parameters fail to decode with `InvalidParams`
(-32602) and keep running.

#### Scenario: Malformed definition params
- **WHEN** a `textDocument/definition` request has no `position`
- **THEN** the response carries error code -32602
