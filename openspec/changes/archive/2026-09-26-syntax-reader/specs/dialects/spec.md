# Spec Delta

## Purpose

Describe each Lisp dialect as data (reader rules, definition and binding forms, indentation,
builtins) so behavior is configurable without code changes and new dialects can be added.

## ADDED Requirements

### Requirement: Built-in dialects
The server SHALL ship definitions for `common-lisp`, `clojure`, `scheme`, `racket`,
`emacs-lisp`, `fennel` and `janet`.

#### Scenario: Built-in list
- **WHEN** built-in dialects are loaded
- **THEN** all seven names are available with file extensions assigned

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
