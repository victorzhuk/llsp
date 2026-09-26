# Spec Delta

## ADDED Requirements

### Requirement: Diagnostics settings
The `diagnostics` section SHALL provide `enable` (true), `debounce_ms` (100),
`unused_binding` ("hint"), `duplicate_definition` ("warning"), `unresolved_call` ("off"),
`ignore_prefix` ("_") and `known_symbols` ([]).

#### Scenario: Invalid severity
- **WHEN** `diagnostics.unused_binding = "loud"`
- **THEN** validation fails naming `unused_binding`
