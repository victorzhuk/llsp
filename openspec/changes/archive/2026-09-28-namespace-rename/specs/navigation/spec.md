# Spec Delta

## MODIFIED Requirements

### Requirement: References and highlights
References SHALL return every occurrence resolving to the same binder (local) or to the same
global name and namespace in same-dialect files, honoring `includeDeclaration`. A global
occurrence resolves to the namespace named by its qualifier (through aliases) or a `:refer`,
else to the current namespace when that namespace defines the name, else to the only
namespace defining it. When the occurrence under the cursor is ambiguous, every same-named
occurrence is returned. Document highlights SHALL return the same-named occurrences in the
current document, marking binders and definition names as writes.

#### Scenario: Local references
- **WHEN** references are requested on binder `x` in `(let ((x 1)) (+ x x))` with includeDeclaration false
- **THEN** two ranges are returned

#### Scenario: Referred name
- **WHEN** `app.a` and `app.b` both define `run`, `app.c` requires `[app.b :refer [run]]`, and references are requested on `(run)` in `app.c`
- **THEN** the result contains the definition in `app.b` and the uses in `app.c`, and nothing from `app.a`

### Requirement: Rename
Prepare-rename SHALL return the base-name range of the symbol under the cursor and refuse
symbols with no binder and no workspace definition, and global symbols whose namespace is
ambiguous. Rename SHALL validate that the new name reads as a single symbol in the dialect
and return edits for every reference resolving to the target's namespace (base names only,
keeping qualifiers) across open and indexed files.

#### Scenario: Rename across files
- **WHEN** `helper` is defined in `a.lisp`, called as `app::helper` in `b.lisp`, and renamed to `assist`
- **THEN** edits replace `helper` in both files, leaving `app::` intact

#### Scenario: Same name in another namespace
- **WHEN** `app.a` and `app.b` both define `run` and `run` is renamed from its definition in `app.a`
- **THEN** edits change `app.a`'s definition and uses, including `a/run` through an alias, and nothing that resolves to `app.b`

#### Scenario: Ambiguous reference
- **WHEN** packages `app` and `lib` both define `helper` and prepare-rename is requested on an unqualified `(helper)` in a file with no package
- **THEN** the response is a RequestFailed error

#### Scenario: Invalid new name
- **WHEN** rename is requested with new name `two words`
- **THEN** the response is an InvalidParams error

#### Scenario: Builtin
- **WHEN** prepare-rename is requested on `car` with no workspace definition
- **THEN** the response is an error explaining there is no definition to rename
