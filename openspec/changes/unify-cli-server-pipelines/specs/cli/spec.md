## ADDED Requirements

### Requirement: Format resolves workspace hints
`llsp format` SHALL resolve declared indent hints across all files it processes, so its
edits match what the server's formatting produces for the same workspace and configuration.

#### Scenario: Hint in another file
- **WHEN** `a.lisp` defines `(defmacro with-x (a &rest body) (declare (indent 1)) body)` and
  `b.lisp` contains `(with-x a\nbody)`, and both live in the directory being formatted
- **THEN** `llsp format .` indents `b.lisp`'s `body` by `format.body_indent`, the same edit
  `textDocument/formatting` returns for `b.lisp`

### Requirement: Shared file budget
`check` and `format` SHALL apply one `workspace.max_files` budget across all directory
arguments, like the server's initial scan, and SHALL report truncation once. Files named
explicitly on the command line are always processed.

#### Scenario: Two directories
- **WHEN** `llsp check dir1 dir2` runs with `workspace.max_files = 10` and the directories
  hold 8 and 20 checkable files
- **THEN** at most 10 files are checked in total and one warning is printed
