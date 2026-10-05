# Tasks

## 1. Dialect bounds

- [x] 1.1 Cap `extends` chain depth and total dialect count in `Dialects::load`/`resolve`
      (`src/dialect.rs`): depth is checked per chain (order-independent, memoized depths)
      and the count before resolution; verify unit tests `deep_extends_chain_is_error`,
      `dialect_count_is_bounded`, `moderate_extends_chain_loads`, and `task test` passes
      unchanged for stock dialects
- [x] 1.2 Verify the integration path: a workspace whose `.llsp.toml` declares a deep chain
      produces the named error through `llsp config` (exit 2), and client-sent settings
      reuse the existing validation failure path (log + keep previous configuration);
      verify `deep_dialect_chain_in_project_file_is_named` in `tests/cli.rs` and the
      client-reload tests in `tests/sync.rs`

## 2. Numeric bounds

- [x] 2.1 Validate `diagnostics.debounce_ms`, `format.body_indent` and
      `format.distinguished_indent` in `Settings::from_table` against `MAX_DEBOUNCE_MS`
      (60 000) and `MAX_INDENT` (1000), rejecting out-of-range values with an error naming
      the key and the bound; verify `numeric_bounds_are_enforced` in `src/config.rs`
      (overflowing, huge, and boundary values; `0` and defaults accepted)
- [x] 2.2 Clamp computed indent targets in `format::run` to `MAX_INDENT` so nested
      generated files cannot allocate unbounded edit text; verify `deep_nesting_stays_bounded`
      formats a 10 000-line `(`-per-line file with total edit text ≤ lines × MAX_INDENT

## 3. Panic isolation

- [x] 3.1 Wrap `didChangeConfiguration`, `didChangeWatchedFiles` and `didClose` notification
      handlers in `catch_unwind` via `notify_isolated` (log and continue), keeping
      `didOpen`/`didChange` unwrapped; verify `state_transition_notifications_keep_server_running`
      in `tests/robustness.rs` issues all three and the server still answers `shutdown`

## 4. Signature labels

- [x] 4.1 Track UTF-16 offsets incrementally in `signature_label` (O(params) total) and cap
      collected parameters per signature at `MAX_SIGNATURE_PARAMS` (1024) in `analysis.rs`;
      verify hover/signature-help tests stay green and `hover_on_huge_parameter_list_answers`
      answers for a 5 000-parameter definition with a capped label

## 5. Log file

- [x] 5.1 Refuse symlinked log paths (pre-open `symlink_metadata` check) and normalize the
      mode to 0600 after open so pre-existing files are covered; verify
      `log_file_mode_is_normalized` and `log_file_rejects_symlink` in `tests/cli.rs`
