## ADDED Requirements

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
