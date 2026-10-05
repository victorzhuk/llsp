# Tasks

## 1. Shared lookup

- [ ] 1.1 Add `Index::defs_in(&self, key, dialect_name, dialect, cell)` returning
      same-dialect, cell-matching definitions; replace the hand-written filter at the eight
      call sites (`src/main.rs:236-240`, `src/server.rs:624-629`,
      `src/features/mod.rs:110-117`, `src/features/navigation.rs:57-66,111-118`,
      `src/features/assist.rs:186-190,291-300`, `src/features/structure.rs:319-324`); verify
      `task test` green and `grep -rn "defs_named" src/` shows no caller repeating the
      dialect/cell predicate

## 2. Format parity

- [ ] 2.1 Add a shared hints helper (e.g. `workspace::indent_hints(index, dialect)`) used by
      both `format_edits` (`src/features/formatting.rs:33-38`) and `format_files`; verify a
      two-file test where `a.lisp` declares `(declare (indent 1))` and `b.lisp` uses the
      macro: `textDocument/formatting` and `llsp format b.lisp` produce the same
      `body_indent` indentation

## 3. File budget

- [ ] 3.1 Make `collect_files` accumulate all path arguments under one
      `workspace.max_files` budget with one truncation warning; verify a test with two
      directories totalling more than `max_files` processes exactly `max_files` files and
      prints one warning
