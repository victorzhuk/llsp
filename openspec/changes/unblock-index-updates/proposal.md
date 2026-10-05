# Proposal

## Why

The workspace-index spec's "without blocking request handling" guarantee covers only the
initial scan, but three verified paths do index work on the synchronous main loop and block
every request:

- A batch of `didChangeWatchedFiles` events (git checkout, build output) is summarized one
  file at a time — full disk read + parse + analysis per file — serially on the main loop
  (`src/server.rs:497-519`, `src/workspace.rs:227-236`); N touched files stall all requests
  for N × parse time.
- `didClose` re-reads and re-parses the file from disk on the main loop to restore the
  index entry (`src/server.rs:479-495`).
- Completion materializes a `Candidate` — name, signature label, docstring clones — for
  every workspace match before ranking and truncating to `completion.max_items`
  (`src/features/assist.rs:88-136, 389-408`): at the advertised 50k-definition scale that is
  megabytes of allocation plus a 50k-element sort per keystroke. `workspace_symbols` shows
  the correct rank-then-materialize pattern (`src/features/symbols.rs:57-70`).

## What Changes

- Watched-file event batches and close-restore go through the existing off-loop scan worker
  (`Event::Indexed` pattern), coalesced, with the open-document guard kept; single-file
  updates stay on-loop when cheap.
- Completion collects borrowed scores, sorts, dedups and truncates first, cloning only the
  surviving `max_items` candidates.
- Micro-fixes on measured paths: `document_highlight` precomputes the definition-name
  offsets instead of scanning all definitions per occurrence; `format::apply` builds output
  in one pass instead of `replace_range` per edit; `workspace_symbols` keeps its
  bounded collection.

## Capabilities

### Modified Capabilities

- `workspace-index`: the non-blocking guarantee extends to live index updates.

## Impact

`src/server.rs`, `src/workspace.rs`, `src/features/assist.rs`,
`src/features/navigation.rs`, `src/format.rs`. Observable change: requests answered during
index bursts instead of queued behind them; completion allocations bounded by
`completion.max_items` regardless of workspace size. The completion bench
(`requests_50000_defs/completion`) is the regression guard.
