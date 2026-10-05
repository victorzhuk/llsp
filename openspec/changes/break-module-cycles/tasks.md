# Tasks

## 1. Leaf extractions (no behavior change)

- [x] 1.1 Move `fuzzy_score`, `score_lowercase`, `is_subsequence` to `src/search.rs`;
      `workspace.rs` and `features/*` import from there; verify `task test` and
      `grep -rn "crate::features" src/workspace.rs` is empty
- [x] 1.2 Move `merge_tables` and `json_to_table` to `src/tables.rs`; verify `task test` and
      `grep -rn "crate::config" src/dialect.rs` is empty
- [x] 1.3 Move `impl Settings { detect }` from `src/document.rs:225-264` to `src/config.rs`;
      verify `task test` (the `detection_order` unit test moves with it)

## 2. Session extraction

- [x] 2.1 Extract `Session` (`docs`, `index`, `settings`, `enc`, `roots`) from `Server`;
      feature handlers become free functions taking `&Session`/`&mut Session`; `Server`
      keeps the connection, dispatch and scheduling; verify `task test` and that
      `grep -rn "impl Server" src/features/` is empty
- [x] 2.2 Unit-test one previously untestable policy (`rename_target`,
      `features/navigation.rs:264-289`) directly against a `Session`; verify the new test
      needs no `Connection`

## 3. Error shaping

- [x] 3.1 Replace `HandlerResult`/`ResponseError` in feature handlers with a `FeatureError`
      enum (`NoDefinition`, `Ambiguous`, `InvalidName`); map to LSP codes at the dispatch
      site in `server.rs`; verify `task test` — every error scenario (ambiguous rename,
      invalid new name, builtin rename) returns the same codes as before
