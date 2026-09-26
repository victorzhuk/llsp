# Spec Delta

## Purpose

Let editors navigate Lisp code statically: symbols, definitions, references, highlights and
rename, for local bindings and workspace-wide names.

## ADDED Requirements

### Requirement: Document symbols
The server SHALL return the document's definitions as hierarchical symbols: kind from the
definition kind, range of the whole form, selection range of the name, detail from the first
signature; definitions nested inside another definition's form are its children.

#### Scenario: Outline
- **WHEN** a document defines `(defun a ())` and `(defclass b () ())`
- **THEN** document symbols are `a` (function) and `b` (class)

### Requirement: Workspace symbols
The server SHALL return indexed definitions whose names match the query as a case-insensitive
subsequence, best matches first (exact, prefix, substring, subsequence, then shorter names),
at most `workspace.max_symbols`.

#### Scenario: Fuzzy query
- **WHEN** the workspace defines `make-point`, `point-x` and `mapcar-safe` and the query is `mkp`
- **THEN** the result contains `make-point` only

### Requirement: Definition
Go-to-definition SHALL resolve a local symbol to its binder and a global symbol to indexed
definitions with the same normalized name in files of the same dialect, preferring the current
file and, for qualified symbols, definitions in the qualifier's namespace (after aliases).

#### Scenario: Local
- **WHEN** definition is requested on `x` in the body of `(let ((x 1)) x)`
- **THEN** the result is the binder `x`

#### Scenario: Cross-file
- **WHEN** `b.lisp` calls `(helper)` and `a.lisp` defines `helper`
- **THEN** definition on the call returns the name range in `a.lisp`

#### Scenario: Other dialect ignored
- **WHEN** `a.clj` defines `helper` and `b.lisp` calls `(helper)`
- **THEN** definition returns nothing

### Requirement: References and highlights
References SHALL return every occurrence resolving to the same binder (local) or the same
global name in same-dialect files, honoring `includeDeclaration`. Document highlights SHALL
return the same set limited to the current document, marking binders and definition names
as writes.

#### Scenario: Local references
- **WHEN** references are requested on binder `x` in `(let ((x 1)) (+ x x))` with includeDeclaration false
- **THEN** two ranges are returned

### Requirement: Rename
Prepare-rename SHALL return the base-name range of the symbol under the cursor and refuse
symbols with no binder and no workspace definition. Rename SHALL validate that the new name
reads as a single symbol in the dialect and return edits for every reference (base names only,
keeping qualifiers) across open and indexed files.

#### Scenario: Rename across files
- **WHEN** `helper` is defined in `a.lisp`, called as `app::helper` in `b.lisp`, and renamed to `assist`
- **THEN** edits replace `helper` in both files, leaving `app::` intact

#### Scenario: Invalid new name
- **WHEN** rename is requested with new name `two words`
- **THEN** the response is an InvalidParams error

#### Scenario: Builtin
- **WHEN** prepare-rename is requested on `car` with no workspace definition
- **THEN** the response is an error explaining there is no definition to rename
