# Tasks

## 1. Off-loop index updates

- [ ] 1.1 Route batches of watched-file events through the scan worker (coalesce events,
      summarize off-loop, apply one index update on the main loop, keep the
      `docs.contains_key` guard); verify a test fires 200 events and a hover issued
      immediately after is answered before the batch finishes, then `task test`
- [ ] 1.2 Restore the on-disk index entry on `didClose` through the same mechanism; verify
      the existing close-restore tests (`tests/sync.rs`) stay green

## 2. Completion bounding

- [ ] 2.1 Rank-then-materialize in completion (`src/features/assist.rs:88-136`): collect
      borrowed `(score, rank, key)` tuples, sort, dedup, truncate, then clone only the
      surviving `max_items` candidates; verify completion tests stay green and
      `task bench requests_50000_defs/completion` improves or holds
- [ ] 2.2 Apply the same bounded collection to `workspace_symbols`
      (`src/features/symbols.rs:57-70`); verify the workspace-symbol tests stay green

## 3. Micro-fixes

- [ ] 3.1 Precompute definition-name start offsets per key in `document_highlight`
      (`src/features/navigation.rs:164-179`); verify highlight tests stay green
- [ ] 3.2 Single-pass builder in `format::apply` (`src/format.rs:37-43`) instead of repeated
      `replace_range`; verify `tests/cli.rs::format` tests and the format bench stay green
- [ ] 3.3 Group binders per scope by name key in `analysis.rs` so `resolve` stops filtering
      every binder of every ancestor scope (O(atoms × binders) today: a 50 000-parameter
      definition stalls analysis for seconds — found by the `harden-config-limits` hover
      test); verify the same test file indexes and answers within the harness timeout
