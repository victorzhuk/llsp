# Spec Delta

## ADDED Requirements

### Requirement: Data forms
A dialect definition SHALL list `data_forms`: heads whose arguments hold data such as
clauses, slot specifications or patterns; lists nested inside them are not treated as calls.

#### Scenario: Case clauses
- **WHEN** `unresolved-call` is enabled and `(case x (red 1))` is analyzed in `common-lisp`
- **THEN** `red` is not reported
