# Tasks

## 1. Confinement re-check

- [ ] 1.1 In `summarize` (`src/workspace.rs:227-236`), re-verify the canonicalized path is
      inside the roots immediately before reading; verify a test where a discovered
      directory is swapped to a symlink pointing outside yields no indexed files

## 2. Traceability tests

- [ ] 2.1 TCP: over-the-wire test that `--listen 127.0.0.1:PORT` serves LSP, a second
      connection while one session is active receives no LSP traffic, and the first session
      keeps answering; verify in `tests/lifecycle.rs`
- [ ] 2.2 Watcher registration: initialize with
      `workspace.didChangeWatchedFiles.dynamicRegistration = true` and assert the
      `client/registerCapability` request carries method `workspace/didChangeWatchedFiles`
      and a glob covering the dialect extensions; verify in `tests/lifecycle.rs`
- [ ] 2.3 Fixed roots: send `workspace/didChangeWorkspaceFolders` adding a folder with lisp
      files; assert the server stays alive, answers requests, and does not index the new
      folder; verify in `tests/sync.rs`
- [ ] 2.4 Format JSON: `llsp format --check . | cat` prints a JSON array of paths; verify in
      `tests/cli.rs`
