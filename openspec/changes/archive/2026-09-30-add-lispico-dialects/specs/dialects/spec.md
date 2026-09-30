## MODIFIED Requirements

### Requirement: Built-in dialects
The server SHALL ship definitions for `common-lisp`, `clojure`, `scheme`, `racket`,
`emacs-lisp`, `fennel`, `janet`, `lispico-clojure` and `lispico-cl`. The two Lispico dialects
SHALL claim no file extensions.

#### Scenario: Built-in list
- **WHEN** built-in dialects are loaded
- **THEN** all nine names are available, and every dialect except the Lispico ones has file extensions assigned

## ADDED Requirements

### Requirement: Lispico dialects
`lispico-clojure` and `lispico-cl` SHALL follow go-lispico's reader, special forms, definition
and binding forms, and stdlib builtins, and SHALL be selected by language ID
(`lispico-clojure`, `lispico-cl`) or `files.associations`.

#### Scenario: Language ID
- **WHEN** a client opens `rules/a.clj` with language ID `lispico-clojure`
- **THEN** the document uses `lispico-clojure`, not `clojure`

#### Scenario: CL vocabulary
- **WHEN** `(car xs)` and `(first xs)` are analyzed in `lispico-cl`
- **THEN** both heads are builtins

#### Scenario: Clojure-only syntax
- **WHEN** `#{1 2}` is read in `lispico-clojure`
- **THEN** one `invalid-syntax` error is reported at `#`, the token is a one-byte atom, and the
  following forms in the file still parse

### Requirement: Function cells
A dialect SHALL be able to declare `function_cells = true` and, per definition form, the cell
it binds (`function` or `value`). Local binders SHALL bind the value cell, including in the
`flet` shape; no dialect with `function_cells = true` declares a flet-shape binding, so the
`flet` names in the stock dialects are unaffected because their flag is off.

#### Scenario: Default
- **WHEN** a dialect omits `function_cells`
- **THEN** it has one namespace for all bindings
