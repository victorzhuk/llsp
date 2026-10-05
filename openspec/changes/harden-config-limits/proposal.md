# Proposal

## Why

The security spec promises the server never aborts on hostile input, but three verified
paths break that promise from the untrusted project layer (`.llsp.toml`, client-sent
settings) and bypass the request-level panic guard because they run at startup or inside
notifications:

- `extends` resolution recurses without a depth or count limit and clones the accumulated
  table at every level (`src/dialect.rs:459-488`): a chain of ~50k `[aN] extends = "aN+1"`
  tables aborts the process at startup (stack overflow or OOM).
- `debounce_ms` is an unvalidated `u64` (`src/config.rs:100`); `Instant::now() +
  Duration::from_millis(ms)` (`src/server.rs:600`) panics on overflow, and notifications run
  outside `catch_unwind` — the first edit in any document kills the server.
- Formatter indent targets are unbounded (`" ".repeat(target)`, `src/format.rs:73,77`) with
  unvalidated `body_indent`/`distinguished_indent` (`src/config.rs:124-138`): a planted
  config or an adversarial `(`-per-line file allocates O(n²) bytes.

Also verified: `signature_label` re-counts the whole label's UTF-16 length per parameter
(`src/features/assist.rs:443-454`, O(params²) hang), the log file is opened without
`O_NOFOLLOW` and its 0600 mode applies only at creation (`src/main.rs:161-178`).

## What Changes

- Bounded `extends` resolution: chain depth cap, total dialect count cap, memoized
  resolution; violations fail with an error naming the limit.
- Numeric settings get documented upper bounds enforced by validation (`debounce_ms`,
  `format.body_indent`, `format.distinguished_indent`); computed formatter indent targets
  are clamped.
- Non-sync notifications (`didChangeConfiguration`, `didChangeWatchedFiles`, `didClose`) are
  panic-isolated; `didOpen`/`didChange` stay unwrapped (sync document state).
- Signature labels track UTF-16 offsets incrementally and cap collected parameters.
- Log files open with `O_NOFOLLOW` and have their mode normalized to 0600 on every open.

## Capabilities

### Modified Capabilities

- `configuration`: bounded numeric settings and bounded dialect definitions.
- `security`: resource use bounded for validated inputs; log file hardening.

## Impact

`src/dialect.rs`, `src/config.rs`, `src/server.rs`, `src/format.rs`, `src/features/assist.rs`,
`src/main.rs`, tests in `tests/` and inline modules. Stock configs are unaffected: current
defaults (`debounce_ms = 100`, `body_indent = 2`, `distinguished_indent = 4`) are far below
the bounds. Behavior for out-of-range values changes from crash/hang to a named validation
error.
