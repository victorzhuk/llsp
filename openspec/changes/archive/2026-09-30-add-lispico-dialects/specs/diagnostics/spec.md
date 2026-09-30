## MODIFIED Requirements

### Requirement: Unresolved call
The `unresolved-call` lint SHALL report unqualified symbols in call position that resolve to
no local binder, no same-dialect workspace definition, no dialect special form, builtin or
constant, and no entry of `diagnostics.known_symbols`, ignoring quoted forms. In a dialect with
`function_cells = true`, only function-cell binders and definitions satisfy a call.

#### Scenario: Unknown function
- **WHEN** the lint is enabled and a document calls `(frobnicate 1)` defined nowhere
- **THEN** an `unresolved-call` diagnostic is reported on `frobnicate`

#### Scenario: Quoted data
- **WHEN** the lint is enabled and a document contains `'(frobnicate 1)`
- **THEN** no diagnostic is reported

#### Scenario: Value binding in call position
- **WHEN** the lint is enabled and `(let ((k 1)) (k))` is analyzed in `lispico-cl` with no function `k`
- **THEN** an `unresolved-call` diagnostic is reported on the head `k`
