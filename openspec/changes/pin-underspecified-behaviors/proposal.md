# Proposal

## Why

Several observable behaviors are implemented but specified nowhere, so any refactor can
change them silently and no test can be traced to a requirement:

- TCP trust model: the loopback listener accepts exactly one unauthenticated connection
  (`lsp-server` binds and accepts once, `src/main.rs:180-187`); whichever local process
  connects first owns the session and can read symbol names and paths from the indexed
  workspace. The lsp-lifecycle spec documents loopback-only binding but not the trust model.
- Workspace roots are fixed at initialization; `workspace/didChangeWorkspaceFolders` is
  ignored (no handling anywhere in `src/server.rs`). Nothing says so, and nothing says
  whether later folders are indexed.
- Watcher registration: when the client advertises `didChangeWatchedFiles.dynamicRegistration`
  and `workspace.index` is on, the server registers a `client/registerCapability` watcher
  over all dialect extensions (`src/server.rs:280-306`). Unspecified.
- Confinement is checked at discovery and on watched events, but `summarize` guards only
  the final path component and `scan` never re-checks (`src/workspace.rs:227-251`) — a
  component swapped to a symlink during a scan window can read outside a root. The spec
  should say confinement is enforced at read time.
- `llsp format` supports `--format text|json` (verified: `src/main.rs:295-345`, JSON pretty
  array of changed paths), but the cli spec's machine-readable requirement lists only
  `check`, `config` and `dialects`.

## What Changes

- Spec-only pins: TCP trust model (lsp-lifecycle), fixed roots and watcher registration and
  read-time confinement (workspace-index), `format` in machine-readable output (cli).
- One code fix backing the confinement wording: `summarize` re-checks the canonicalized
  path against the roots immediately before reading.

## Capabilities

### Modified Capabilities

- `lsp-lifecycle`: TCP trust model stated.
- `workspace-index`: fixed roots, watcher registration, read-time confinement.
- `cli`: `format` joins machine-readable output.

## Impact

One-line-ish code change in `src/workspace.rs` plus tests; all other work is spec text and
traceable tests. No behavior change except refusing reads whose canonical path escapes the
roots (today reachable only through a racing symlink swap).
