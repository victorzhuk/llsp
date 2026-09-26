# Spec Delta

## Purpose

Expose the structure of Lisp code to editors: what can fold, how selection grows, and what
each token means for highlighting.

## ADDED Requirements

### Requirement: Folding ranges
The server SHALL return a folding range for every form spanning more than one line (from its
first to its last line), every multi-line block comment and every run of two or more
consecutive line comments (kind comment).

#### Scenario: Nested forms
- **WHEN** a document has a three-line `defun` containing a two-line `let`
- **THEN** both forms fold

### Requirement: Selection ranges
For each requested position the server SHALL return the chain of ranges from the innermost
atom or form at the position outward through every enclosing form.

#### Scenario: Expand selection
- **WHEN** selection ranges are requested on `b` in `(a (b c))`
- **THEN** the chain is `b`, `(b c)`, `(a (b c))`

### Requirement: Semantic tokens
The server SHALL provide full-document and range semantic tokens with a legend of types
(namespace, type, function, macro, variable, parameter, property, keyword, comment, string,
number, regexp) and modifiers (declaration, definition, readonly, defaultLibrary). Atoms
SHALL be classified as: numbers; keywords (`:k`) as property; constants as variable with
readonly and defaultLibrary; special forms as keyword; builtins as function with
defaultLibrary; definition names by definition kind with definition; binders as parameter
with declaration; local references as parameter; other symbols by the kind of their
workspace definition; qualifiers as namespace. Tokens spanning lines SHALL be split per line.

#### Scenario: Classification
- **WHEN** tokens are requested for `(defun f (x) (car x)) ; c`
- **THEN** `defun` is keyword, `f` is function+definition, first `x` is parameter+declaration, `car` is function+defaultLibrary, second `x` is parameter, `; c` is comment
