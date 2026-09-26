# Tasks

## 1. Engine

- [x] 1.1 Adjust dialect indent tables (`if` in Common Lisp and Clojure); verify dialect tests
- [x] 1.2 Implement line-based re-indent with shifts, literals untouched and trailing trim; verify unit tests per scenario and dialect
- [x] 1.3 Add proptest for idempotence and whitespace-only changes; verify `task test`

## 2. Server and CLI

- [x] 2.1 Formatting and range formatting handlers with workspace indent hints; verify integration tests
- [x] 2.2 `llsp format [--check]`; verify CLI tests
- [x] 2.3 Add format benchmark and README section; verify `cargo bench --no-run`
