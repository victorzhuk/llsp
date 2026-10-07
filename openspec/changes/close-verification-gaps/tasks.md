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

- [x] 2.1 Add `\r\n` and `\r` to the fuzz strategies (`src/syntax/tests.rs::lisp_ish`,
      `tests/robustness.rs` document strategy) so every 9-dialect proptest covers CRLF;
      verify `task test`
- [x] 2.2 Unit tests for CRLF position math in `src/document.rs` (`crlf_positions_round_trip`:
      offsets/positions in UTF-16 across CRLF, clamping past line content), CRLF formatting
      in `src/format.rs` (`crlf_lines_keep_their_carriage_returns`: CR survives, trailing
      spaces before it trimmed); the didOpen e2e with CRLF is covered by the robustness
      fuzz now generating `\r` through every request path

## 3. P1 — hostile state transitions

- [x] 3.1 Overlapping ranged changes in one `didChange`, an inverted range (`start > end`),
      and a UTF-16 change landing mid-surrogate: assert resulting text, tree
      well-formedness, no panic; verify `overlapping_and_inverted_edits_do_not_panic`
      in `src/document.rs` tests
- [x] 3.2 Startup config error paths: `--config` with garbage exits 2 naming the file
      (`unreadable_config_file_is_named`), garbage `.llsp.toml` fails the CLI naming the
      file (`garbage_project_file_fails_the_cli_but_names_it`) while the server warns and
      continues (`garbage_project_file_warns_and_continues` in `tests/sync.rs`), and the
      scalar-then-nested env conflict is named (`scalar_then_nested_env_conflict_is_named`
      in `src/config.rs` — CLI-level spawn blocked by the same hook false positive)
- [x] 3.3 Robustness fuzz breadth: the proptest loop additionally issues `didClose` +
      reopen, random `didChangeWatchedFiles` (including outside-root URIs and delete
      events), a `didChangeConfiguration` reload, and rename with an invalid name
      asserting -32602; verify `every_request_survives_random_documents` stays green
      over 9 dialects × 48 cases

## 4. P1 — unasserted spec details

- [x] 4.1 Hover: `file` field shown (`a.lisp:2` in `hover_kinds`), same name in two
      same-dialect files joined into two blocks (`hover_joins_same_name_from_two_files`),
      qualified hover narrows to the qualifier's namespace
      (`qualified_hover_narrows_to_the_namespace`)
- [x] 4.2 Completion: same name as builtin + workspace def + local yields exactly one
      item; the binder under the cursor is not offered
      (`completion_labels_appear_once_and_binder_self_is_excluded`)
- [x] 4.3 References on an ambiguous unqualified global return every same-named occurrence
      (`references_on_ambiguous_global_return_every_occurrence`)

## 5. P2 — dead code

- [x] 5.1 Delete `Analysis::binder_index_at`, `Tree::token_index_at`, `Document::lines`
      after confirming zero call sites with grep (`dialect::yes`/`dialect::parens` stay:
      they are serde default providers, not dead code); verify
      `cargo build && cargo clippy -- -D warnings && task test`
