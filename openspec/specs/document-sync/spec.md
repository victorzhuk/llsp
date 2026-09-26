# document-sync Specification

## Purpose
Keep the server's copy of each open document identical to the client's and read it with
the right dialect.

## Requirements

### Requirement: Incremental sync
The server SHALL advertise incremental sync and apply ranged and full-text changes in order.

#### Scenario: Ranged edit
- **WHEN** `(a b)` is open and a change replaces `b` with `c d`
- **THEN** the stored text is `(a c d)`

#### Scenario: Out-of-range edit
- **WHEN** a change range lies beyond the end of the document
- **THEN** the range is clamped to the document end and the server keeps running

### Requirement: Dialect detection
The server SHALL choose a document's dialect by, in order: configured file associations
(glob to dialect), the client's `languageId`, a `#lang` line or `-*- mode: X -*-` modeline
in the first line, the file extension, then the configured default dialect.

#### Scenario: Association wins
- **WHEN** config maps `*.lsp` to `emacs-lisp` and `a.lsp` opens with languageId `lisp`
- **THEN** the document is read as `emacs-lisp`

#### Scenario: Modeline
- **WHEN** `script` (no extension) opens with first line `;; -*- mode: clojure -*-` and an unknown languageId
- **THEN** the document is read as `clojure`

### Requirement: Close
The server SHALL drop a document's text on `didClose` and clear its diagnostics.

#### Scenario: Close clears diagnostics
- **WHEN** a document with syntax errors is closed
- **THEN** the server publishes an empty diagnostics list for it
