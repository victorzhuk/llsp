# Spec Delta

## ADDED Requirements

### Requirement: Check runs lints
`llsp check` SHALL report the same lints as the server, resolving names across all files
being checked, with the same configuration.

#### Scenario: Cross-file call
- **WHEN** `llsp check --set diagnostics.unresolved_call=\"warning\" .` runs on `a.lisp` defining `helper` and `b.lisp` calling it
- **THEN** no `unresolved-call` is reported for `helper`
