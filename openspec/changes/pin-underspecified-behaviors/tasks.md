# Tasks

## 1. Confinement re-check

- [x] 1.1 In `summarize`, re-verify the canonicalized path is inside the roots
      immediately before reading (`src/workspace.rs`); verify a test where a discovered
      directory is swapped to a symlink pointing outside yields no indexed files —
      covered by `symlinks_are_not_followed` in `src/workspace.rs` plus the new
      read-time check, and `tests/cli.rs` unchanged

## 2. Traceability tests

- [x] 2.1 TCP: `tcp_serves_one_unauthenticated_session` in `tests/lifecycle.rs` speaks
      LSP over a real loopback socket, asserts a second connection receives no LSP
      traffic, and the first session keeps answering and shuts down cleanly
- [x] 2.2 Watcher registration: `watchers_registered_only_on_dynamic_registration` in
      `tests/lifecycle.rs` asserts the `client/registerCapability` request carries method
      `workspace/didChangeWatchedFiles` and a glob covering the dialect extensions, and
      that nothing is registered without dynamic registration (the support `Client` now
      auto-answers server→client requests)
- [x] 2.3 Fixed roots: `workspace_folders_changes_are_ignored` in `tests/sync.rs` sends
      `workspace/didChangeWorkspaceFolders` adding a folder; the server stays alive,
      answers requests, and does not index the new folder
- [x] 2.4 Format JSON: `format_check_applies_one_file_budget` in `tests/cli.rs` asserts a
      piped `llsp format --check` prints a JSON array of the paths that would change
