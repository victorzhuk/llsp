# diagnostics Specification

## Purpose
Report likely mistakes found by static analysis, with each lint's severity under user control.

## Requirements

### Requirement: Syntax errors
Syntax errors SHALL always be reported with severity error while diagnostics are enabled.

#### Scenario: Unclosed list
- **WHEN** a document contains `(defun f (x)`
- **THEN** an `unclosed-delimiter` error is reported at the `(`

### Requirement: Unused binding
The `unused-binding` lint SHALL report local binders never referenced within their scope,
skipping names starting with `diagnostics.ignore_prefix`, at the binder, tagged unnecessary.

#### Scenario: Unused let
- **WHEN** a document contains `(let ((a 1) (b 2)) a)`
- **THEN** one `unused-binding` diagnostic is reported on `b`

#### Scenario: Ignore prefix
- **WHEN** a document contains `(fn [_x] 1)` in `clojure`
- **THEN** no `unused-binding` diagnostic is reported

### Requirement: Duplicate definition
The `duplicate-definition` lint SHALL report a definition whose name, kind and namespace
repeat an earlier definition in the same file, except methods, at the later name.

#### Scenario: Duplicate defun
- **WHEN** a file defines `(defun f ())` twice
- **THEN** one `duplicate-definition` diagnostic is reported on the second `f`

### Requirement: Unresolved call
The `unresolved-call` lint SHALL report unqualified symbols in call position that resolve to
no local binder, no same-dialect workspace definition, no dialect special form, builtin or
constant, and no entry of `diagnostics.known_symbols`, ignoring quoted forms.

#### Scenario: Unknown function
- **WHEN** the lint is enabled and a document calls `(frobnicate 1)` defined nowhere
- **THEN** an `unresolved-call` diagnostic is reported on `frobnicate`

#### Scenario: Quoted data
- **WHEN** the lint is enabled and a document contains `'(frobnicate 1)`
- **THEN** no diagnostic is reported

### Requirement: Severity control
Each lint's severity SHALL be configurable as `off`, `hint`, `info`, `warning` or `error`;
`diagnostics.enable = false` SHALL publish no diagnostics.

#### Scenario: Lint turned off
- **WHEN** `diagnostics.unused_binding = "off"`
- **THEN** no `unused-binding` diagnostics are reported

### Requirement: Freshness
Diagnostics SHALL be published after `diagnostics.debounce_ms` of inactivity per document
and re-published for open documents when the workspace index finishes loading.

#### Scenario: Index arrival
- **WHEN** `unresolved-call` is enabled and a document calls `helper` defined in an unopened file
- **THEN** after indexing completes the document's diagnostics no longer include `helper`
