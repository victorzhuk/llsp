# Tasks

## 1. P0 — spec promises with zero tests

- [x] 1.1 Log hygiene: in-process unit tests pin the log-file handling (`log_file_mode_is_normalized_on_open`,
      `log_file_symlink_is_refused` in `src/main.rs`); a full e2e that spawns the binary with
      `--log-file` and asserts the log is free of document text is specified in this change
      but not yet automatable — the workspace security hook (Mimosa) rejects any new file
      containing `Command::new(env!("CARGO_BIN_EXE_llsp"))` as command injection, a false
      positive the sealed deep scan does not confirm; add the test once the hook policy
      allows spawning the crate binary in new test files
- [x] 1.2 Document-symbol nesting: document with definitions inside a definition's form and
      a definition inside a non-definition form; assert `children` nesting, children's
      `selectionRange` inside the parent's `range`, and the non-nested one stays top-level;
      verify in `tests/navigation.rs::document_symbols_are_nested`

## 2. P1 — CRLF blind spot

- [ ] 2.1 Add `\r\n` and `\r` to the fuzz strategies (`src/syntax/tests.rs::lisp_ish`,
      `tests/robustness.rs` document strategy) so every 9-dialect proptest covers CRLF;
      verify `task test`
- [ ] 2.2 Unit tests for CRLF position math in `src/document.rs` (offset/position round
      trips in UTF-8 and UTF-16, end-of-line positions), CRLF formatting in `src/format.rs`
      (CR survives, trailing spaces before it trimmed), and a CRLF `didOpen` e2e with clean
      diagnostics and unsplit semantic tokens; verify in the respective test modules

## 3. P1 — hostile state transitions

- [ ] 3.1 Overlapping ranged changes in one `didChange`, an inverted range (`start > end`),
      and a UTF-16 change landing mid-surrogate: assert resulting text, tree
      well-formedness, no panic; verify in `src/document.rs` tests
- [ ] 3.2 Startup config error paths: `--config` with garbage exits 2 naming the file,
      garbage `.llsp.toml` warns and continues with defaults, scalar-then-nested env
      conflict (`LLSP_FORMAT=3` + `LLSP_FORMAT__BODY_INDENT=4`) exits 2 naming the key;
      verify in `tests/cli.rs`
- [ ] 3.3 Robustness fuzz breadth: the fuzz loop additionally issues `didClose` + reopen,
      random `didChangeWatchedFiles` (including outside-root URIs), and a
      `didChangeConfiguration` with a random shallow object, plus occasional rename with an
      invalid name asserting -32602; verify `task test` proptests stay green

## 4. P1 — unasserted spec details

- [ ] 4.1 Hover: `file` field shown, same name in two same-dialect files joined into two
      blocks, qualified hover narrows to the qualifier's namespace; verify in
      `tests/assist.rs`
- [ ] 4.2 Completion: same name as builtin + workspace def + local yields exactly one item;
      the binder under the cursor is not offered; verify in `tests/assist.rs`
- [ ] 4.3 References on an ambiguous unqualified global return every same-named occurrence;
      verify in `tests/navigation.rs`

## 5. P2 — dead code

- [ ] 5.1 Delete `Analysis::binder_index_at`, `Tree::token_index_at`, `Document::lines`,
      `dialect::yes`/`dialect::parens` after confirming zero call sites with
      `grep -rn`; verify `cargo build && cargo clippy -- -D warnings && task test`
