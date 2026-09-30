# dialects Specification

## Purpose
Describe each Lisp dialect as data (reader rules, definition and binding forms, indentation,
builtins) so behavior is configurable without code changes and new dialects can be added.

## Requirements

### Requirement: Built-in dialects
The server SHALL ship definitions for `common-lisp`, `clojure`, `scheme`, `racket`,
`emacs-lisp`, `fennel`, `janet`, `lispico-clojure` and `lispico-cl`. The two Lispico dialects
SHALL claim no file extensions.

#### Scenario: Built-in list
- **WHEN** built-in dialects are loaded
- **THEN** all nine names are available, and every dialect except the Lispico ones has file extensions assigned

### Requirement: Reader rules as data
A dialect definition SHALL declare its reader rules: bracket pairs, line comment start,
block comment delimiters and nesting, datum comment prefixes, character literal prefix,
symbol bars, symbol escapes, commas as whitespace, string long-delimiters, and prefixes with
form counts.

#### Scenario: Janet comments
- **WHEN** reading `# note` with the `janet` dialect
- **THEN** the text is one line-comment token

#### Scenario: Clojure commas
- **WHEN** reading `{:a 1, :b 2}` with `clojure`
- **THEN** commas are whitespace and the map holds four atoms

#### Scenario: Emacs Lisp characters
- **WHEN** reading `?\(` with `emacs-lisp`
- **THEN** it is one character token and no list is opened

### Requirement: Language knowledge as data
A dialect definition SHALL declare definition forms (name position, parameter position,
symbol kind), binding forms (by shape), namespace forms, indentation specs, special forms,
builtins, case sensitivity and namespace separators.

#### Scenario: Custom definition form
- **WHEN** a user adds `defroute` as a function definition form
- **THEN** `(defroute home [] ...)` defines `home`

### Requirement: Extension and override
Users SHALL be able to override any field of a built-in dialect and define new dialects
that extend an existing one, with tables merged and later values winning.

#### Scenario: New dialect via extends
- **WHEN** config defines dialect `lfe` with `extends = "common-lisp"` and extensions `["lfe"]`
- **THEN** `.lfe` files read with Common Lisp rules plus the overrides

#### Scenario: Extends cycle
- **WHEN** dialect `a` extends `b` and `b` extends `a`
- **THEN** loading fails with an error naming the cycle

### Requirement: Clause binding shape
The `clauses` binding shape SHALL treat each form after the head as `(params body...)`,
binding the params within that clause.

#### Scenario: case-lambda
- **WHEN** analyzing `(case-lambda ((x) x) ((x y) y))` in `scheme`
- **THEN** each clause binds its own parameters

### Requirement: List parameter search
Definition specs SHALL accept `params = "list"`, meaning the first parenthesized list after
the name holds the parameters.

#### Scenario: defmethod qualifiers
- **WHEN** analyzing `(defmethod area :around ((s square)) (side s))` in `common-lisp`
- **THEN** `s` is a parameter binder and `side s` refers to it

### Requirement: Iterator binding shape
The `iterator` binding shape SHALL bind every element of the bracketed form at position 1
except the last, which is the iterated expression.

#### Scenario: Fennel each
- **WHEN** analyzing `(each [k v (pairs t)] (print k v))` in `fennel`
- **THEN** `k` and `v` are binders and `pairs`, `t` are not

### Requirement: Data forms
A dialect definition SHALL list `data_forms`: heads whose arguments hold data such as
clauses, slot specifications or patterns; lists nested inside them are not treated as calls.

#### Scenario: Case clauses
- **WHEN** `unresolved-call` is enabled and `(case x (red 1))` is analyzed in `common-lisp`
- **THEN** `red` is not reported

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
