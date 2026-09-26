# Tasks

## 1. Dialect data

- [x] 1.1 Add `clauses` and `iterator` shapes and `list` parameter search; switch `case-lambda`, `match-lambda`, `defmethod`, `cl-defmethod` to them; verify dialect unit tests

## 2. Analysis

- [x] 2.1 Definitions: names (symbol, list head, setf, metadata prefix), kinds, signatures, docstrings, auto prefixes; verify unit tests per dialect scenario
- [x] 2.2 Namespaces and aliases; verify unit tests for in-package, ns and :as
- [x] 2.3 Scopes, binders, patterns and resolution; verify unit tests for shadowing, init visibility, destructuring, clauses, defmethod params
- [x] 2.4 Occurrence filtering and normalization; verify unit tests for keywords, numbers, constants, datum comments, case folding
- [x] 2.5 Indentation hints; verify unit tests for declare and :style/indent

## 3. Index

- [x] 3.1 `FileSummary` and `Index` with queries by name; verify unit tests
- [x] 3.2 Background scan with excludes, gitignore, limits and no symlinks; verify tests with a temp workspace
- [x] 3.3 Server integration: open-document overrides, close restore, watched-file registration and events with confinement; verify integration tests
