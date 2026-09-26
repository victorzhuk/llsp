# Tasks

## 1. Limits and safety

- [x] 1.1 Oversized open documents: skip analysis, empty features, one warning; verify integration test
- [x] 1.2 Log file mode and no document text in logs; verify CLI test for 0600 and a grep check over log calls
- [x] 1.3 MSRV 1.88 in Cargo.toml and CI; verify `cargo +1.88 check --locked`

## 2. Robustness

- [x] 2.1 Proptest over the protocol: random documents per dialect, all requests at random positions; verify `task test`
- [x] 2.2 Read-eval document scenario: all features on `#.(...)` input; verify integration test

## 3. Performance

- [x] 3.1 `benches/workspace.rs`: cold index of 1000 files and request latency on a 50k-definition workspace; verify `cargo bench --no-run` and record results
- [x] 3.2 README: security model and benchmark table; verify numbers come from the recorded run
