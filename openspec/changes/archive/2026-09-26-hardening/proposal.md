# Proposal

## Why

The server reads untrusted code all day. Before calling v1 done it needs explicit safety
guarantees, limits on oversized input, proof that no input or request crashes it, and
end-to-end benchmarks that keep it fast.

## What Changes

- Open documents over `files.max_file_size` keep their text but are not analyzed; features
  return empty results and the user is warned once per document.
- Randomized robustness tests: arbitrary text in every dialect, every request at arbitrary
  positions, over the real protocol.
- Benchmarks: cold workspace indexing, and request round-trip latency (completion,
  references, workspace symbols, hover, semantic tokens) on a large synthetic workspace.
- MSRV corrected to 1.88 (let-chains) and checked in CI.
- README documents the security model and benchmark results.

## Capabilities

### New Capabilities

- `security`: execution, input-size and logging guarantees.

### Modified Capabilities

None.

## Impact

`src/document.rs`, `src/server.rs`, new `tests/robustness.rs`, `benches/workspace.rs`, README.
