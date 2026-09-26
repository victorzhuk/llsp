# formatting Specification

## Purpose
Re-indent Lisp code by data-driven rules without changing anything but leading and trailing
whitespace.

## Requirements

### Requirement: Only whitespace changes
Formatting SHALL change only leading whitespace of lines and, when
`format.trim_trailing_whitespace` is true, trailing whitespace; it SHALL NOT touch lines
that start inside a string or block comment, and SHALL be idempotent.

#### Scenario: Multi-line string
- **WHEN** formatting `(f "a\n   b")`
- **THEN** the line `   b")` is unchanged

#### Scenario: Idempotent
- **WHEN** a document is formatted twice
- **THEN** the second pass produces no edits

### Requirement: Indentation rules
Formatting SHALL indent each line by the innermost list open before its first character:
one column past the opener for `[]`/`{}` lists, for a head on its own line, or for non-symbol
heads; for a head with indent spec N, arguments before N at opener column plus
`format.distinguished_indent` and the rest at opener column plus `format.body_indent`;
otherwise align with the first argument when it is on the head's line, else one column past
the opener. Lines outside any list have no indentation.

#### Scenario: Body indentation
- **WHEN** formatting `(defun f (x)\n(let ((y x))\ny))` in `common-lisp`
- **THEN** the result is `(defun f (x)\n  (let ((y x))\n    y))`

#### Scenario: Call alignment
- **WHEN** formatting `(foo a\nb)` and `(foo\nb)`
- **THEN** `b` aligns under `a` in the first and one column past `(` in the second

#### Scenario: Declared indent
- **WHEN** `(defmacro with-x (a &rest body) (declare (indent 1)) ...)` is in the workspace and `(with-x a\nbody)` is formatted
- **THEN** `body` is indented by `format.body_indent`

### Requirement: Range formatting
Range formatting SHALL return edits only for lines intersecting the range, computed as if
the whole document were formatted.

#### Scenario: Single line
- **WHEN** range formatting covers only the last line of a badly indented form
- **THEN** only that line's edit is returned
