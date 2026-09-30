# syntax-reader Specification

## Purpose
Read Lisp source of any configured dialect into a lossless, error-tolerant syntax tree that
every language feature builds on, without evaluating any code.

## Requirements

### Requirement: Lossless reading
The reader SHALL produce tokens whose concatenated text equals the input exactly, including
whitespace, comments and malformed input.

#### Scenario: Round trip
- **WHEN** any UTF-8 text is read with any dialect
- **THEN** concatenating token texts in order yields the original text

### Requirement: Error tolerance
The reader SHALL never panic and SHALL return a tree plus syntax errors for malformed input:
unclosed delimiter, unexpected closer, mismatched closer, unterminated string, unterminated
block comment, and prefix without a following form.

#### Scenario: Unclosed list
- **WHEN** reading `(defun f (x)`
- **THEN** the tree contains the list and an error "unclosed delimiter" located at the outer `(`

#### Scenario: Mismatched closer
- **WHEN** reading `(foo]` in a dialect with brackets
- **THEN** an error "mismatched delimiter" is reported at `]`

#### Scenario: Arbitrary input
- **WHEN** reading random bytes decoded as UTF-8
- **THEN** reading completes without panic

### Requirement: Deep nesting without recursion
The reader SHALL handle arbitrarily deep nesting without stack overflow.

#### Scenario: Deep nesting
- **WHEN** reading 100000 nested `(`
- **THEN** reading completes and at most 100 syntax errors are reported

### Requirement: No evaluation
The reader MUST NOT evaluate code; read-time evaluation syntax (`#.`, `#=`), feature
expressions (`#+`, `#-`) and reader conditionals (`#?`) SHALL be represented as syntax only.

#### Scenario: Read-eval syntax
- **WHEN** reading `#.(delete-file "x")`
- **THEN** the result is a prefixed form node and nothing is executed

### Requirement: Form structure
The tree SHALL expose lists (with their delimiter), atoms, strings, characters, prefixed
forms (quote, quasiquote, unquote, dispatch prefixes with a per-prefix form count), and
datum comments, each with byte ranges and parent links.

#### Scenario: Prefix with two forms
- **WHEN** reading `#+sbcl (foo)` in Common Lisp
- **THEN** a prefix node `#+` holds the atom `sbcl` and the list `(foo)`

#### Scenario: Datum comment
- **WHEN** reading `#_ (foo) bar` in Clojure
- **THEN** `(foo)` sits under a datum-comment node and `bar` is a top-level form

### Requirement: Invalid openers
A dialect's `reader.invalid` list SHALL name the token openers it rejects. The reader SHALL test
a declared opener before every other rule for that byte, so the byte is never classified `OPEN`
or `CLOSE` and never starts a prefix, datum comment, block comment or sharp dispatch. Each
occurrence SHALL be read as one `Atom` token spanning exactly that byte and SHALL produce one
`invalid-syntax` error over that token's own range, so the text stays lossless and the rest of
the file still parses. A dialect with an empty `reader.invalid` SHALL keep the classification it
derives from `brackets` and the other reader rules for every byte.

#### Scenario: Brackets in Lispico CL
- **WHEN** reading `(f [x])` with `lispico-cl`
- **THEN** one `invalid-syntax` error is reported at `[` and one at `]`, both tokens are atoms
  spanning one byte, and the list `(f ...)` stays intact

#### Scenario: Brace pair in Lispico CL
- **WHEN** reading `(f {x})` with `lispico-cl`
- **THEN** `invalid-syntax` errors are reported at `{` and `}`, and the outer list survives

#### Scenario: Hash in Lispico Clojure
- **WHEN** reading `#{1 2}` with `lispico-clojure`
- **THEN** one `invalid-syntax` error is reported at `#` and the token is a one-byte atom, so the
  following forms in the file still parse

#### Scenario: Allowed dispatch
- **WHEN** reading `#'f` and `#(1 2)` with `lispico-cl`
- **THEN** no error is reported; `#'` prefixes one form and `#(` opens a list

#### Scenario: Stock dialects unchanged
- **WHEN** reading `[x]` with `common-lisp` or `{x}` with `clojure`
- **THEN** the behavior is today's, with no `invalid-syntax` error from this list

#### Scenario: Dispatch forms in Lispico Clojure
- **WHEN** reading `#'f` with `lispico-clojure`
- **THEN** the leading `#` is one `invalid-syntax` `Atom` token spanning exactly that byte, no
  `#'` prefix token is produced, and the remaining bytes are read by the dialect's own rules,
  giving a `Prefix` token `'` followed by the `Atom` `f`

#### Scenario: Sharp dispatch opener in Lispico Clojure
- **WHEN** reading `#(1 2)` with `lispico-clojure`
- **THEN** the leading `#` is one `invalid-syntax` `Atom` token spanning exactly that byte, no
  sharp dispatch occurs at that byte, and the remaining bytes are read by the dialect's own
  rules, giving the list `(1 2)`
