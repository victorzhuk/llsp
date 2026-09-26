# Design

## Context

See proposal.md. Syntax trees and dialect data exist; the server keeps open documents only.

## Goals / Non-Goals

Goals: one linear pass per file; compact per-file summaries for closed files; no global
locks on the request path. Non-goals: macro expansion, type inference, cross-file local
scopes, incremental indexing of a single edited form.

## Decisions

- **Two views of a file.** `Analysis` (open documents): definitions, scopes, binders and every
  symbol occurrence with its resolution. `FileSummary` (every indexed file): definitions,
  global reference ranges per name, namespace, aliases and a line index, without the tree.
  Open documents build both on each change; closed files keep only the summary.
- **Resolution** by walking ancestors of each occurrence node through a node→scope map and
  picking the innermost binder whose visibility starts before the occurrence. Visibility starts
  at the end of the binding item (pair, parameter list), which gives let*-like semantics:
  close to real behavior for every dialect with one rule.
- **Patterns**: symbols anywhere in a pattern bind, except keywords, numbers, constants and
  `pattern_ignore` markers; after a `&`-marker, a parenthesized item binds only its first
  element (`&optional (y 1)`). Duplicate names within one pattern bind once.
- **Index storage**: `FxHashMap<Uri, Arc<FileSummary>>` owned by the main loop; each summary
  has a name→definitions map. Queries scan per-file maps (thousands of hash lookups, well
  under a millisecond); no global inverted index to keep consistent.
- **Scanning** on a background thread using `ignore::WalkBuilder` (gitignore, no symlinks) and
  `rayon` for parsing; results come back to the main loop over a channel, skipping open files.
- **Watched files** use dynamic registration when the client supports it, with a glob built
  from dialect extensions. Paths are canonicalized and must lie under a canonical root.
- **URI keys** are normalized through path round-trip so editor and walker URIs match.

## Risks / Trade-offs

- [let init sees earlier binders in plain `let`] → accepted; affects only shadowed names in
  init expressions.
- [Auto `def_prefixes` misread a non-defining `def*` call] → name must be a symbol; prefixes
  are configurable per dialect.
