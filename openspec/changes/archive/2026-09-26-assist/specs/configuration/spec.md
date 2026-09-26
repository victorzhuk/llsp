# Spec Delta

## ADDED Requirements

### Requirement: Completion settings
`completion.max_items` (default 200) SHALL cap completion results and `completion.builtins`
(default true) SHALL control whether dialect special forms and builtins are offered.

#### Scenario: Builtins disabled
- **WHEN** `completion.builtins = false` and completing `ca` in Common Lisp
- **THEN** `car` is not offered unless defined in the workspace
