# Tasks

## 1. Off-loop index updates

- [x] 1.1 Route batches of watched-file events through a worker (queue, coalesce by URI,
      one batch in flight, apply in order with the open-document guard); verify
      `requests_are_served_during_index_bursts`: a request is answered while a batch runs
      and a second is queued, and draining applies every batch in order (`src/server.rs`)
- [x] 1.2 Restore the on-disk index entry on `didClose` through the same mechanism;
      verify the existing close-restore test (`open_document_overrides_disk_and_close_restores`,
      now draining one batch) and the rest of `task test` stay green

## 2. Completion bounding

- [x] 2.1 Rank-then-materialize in completion: collect borrowed `(score, rank, name,
      source)` matches, sort, dedup on the normalized name, truncate, and clone only the
      surviving `max_items` candidates; verify completion tests stay green and
      `requests_50001_defs/completion` benchmarks at ~457 µs (bounded, no per-candidate
      docstring cloning)
- [x] 2.2 `workspace_symbols` was verified to already materialize late (borrowed tuples,
      sort, truncate, then clone), so no change is needed; the remaining O(index) scan per
      query is the same as completion's and is bounded work per request

## 3. Micro-fixes

- [x] 3.1 Precompute definition-name start offsets per key in `document_highlight`;
      verify highlight tests stay green
- [x] 3.2 Single-pass builder in `format::apply` instead of repeated `replace_range`;
      verify `tests/cli.rs::format` tests and the format bench stay green
- [x] 3.3 Group binders per scope by name key and dedup patterns with a hash set, so
      analysis of a 50 000-parameter definition drops from seconds (quadratic) to tens of
      milliseconds (linear); measured 2.4 s → 59 ms at 20 000 params; verify
      `hover_on_huge_parameter_list_answers` at 50 000 params
