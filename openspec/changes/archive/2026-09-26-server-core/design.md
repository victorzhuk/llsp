# Design

## Context

See proposal.md. The reader (`syntax-reader`, `dialects`) is in place; nothing speaks LSP yet.

## Goals / Non-Goals

Goals: one synchronous main loop; configuration merged once per change; identical dialect
detection in LSP and CLI. Non-goals: request cancellation (every handler is sub-millisecond
on realistic files, measured by benches; `$/cancelRequest` is accepted and ignored),
pull-based `workspace/configuration`.

## Decisions

- **`lsp-server` + `lsp-types`.** rust-analyzer's transport: crossbeam channels, no async
  runtime. Handlers run on the main thread in arrival order; heavy work (workspace indexing,
  later) runs on background threads that post results back through a channel.
- **Configuration as merged TOML tables.** Each layer (defaults, user file, project file,
  env, CLI, init options, client settings) is a `toml::Table`; layers deep-merge in order and
  the result deserializes once into `Config` with `deny_unknown_fields`. Client JSON converts
  to TOML (nulls dropped). No partial structs, no config framework.
- **Env mapping** `LLSP_A__B_C=v` → `a.b_c = v`, value parsed as a TOML literal, else a string.
- **Documents** own their text inside the syntax tree; an edit takes the text out, splices,
  and reparses. A `line_index::LineIndex` per document converts between byte offsets and
  UTF-8/UTF-16 positions; out-of-range positions clamp.
- **URIs** convert to paths only for `file:` URIs, with percent-decoding; other schemes stay
  in-memory documents with extension-based detection.
- **Logging** via `log` + `env_logger`, to stderr or a file created with mode 0600.

## Risks / Trade-offs

- [No cancellation] → if later features become slow, add a revision counter; benches guard this.
- [Env keys cannot contain `-` or `.`] → such keys (dialect names, globs) go in files or `--set`
  with quoted TOML tables; documented.
