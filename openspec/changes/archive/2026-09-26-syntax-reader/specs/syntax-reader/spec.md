# Spec Delta

## Purpose

Read Lisp source of any configured dialect into a lossless, error-tolerant syntax tree that
every language feature builds on, without evaluating any code.

## ADDED Requirements

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
