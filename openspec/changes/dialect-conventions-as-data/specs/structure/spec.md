## MODIFIED Requirements

### Requirement: Semantic tokens
The server SHALL provide full-document and range semantic tokens with a legend of types
(namespace, type, function, macro, variable, parameter, property, keyword, comment, string,
number, regexp) and modifiers (declaration, definition, readonly, defaultLibrary). Atoms
SHALL be classified as: numbers; keywords (`:k`) as property; constants as variable with
readonly and defaultLibrary; special forms as keyword; builtins as function with
defaultLibrary; definition names by definition kind with definition; binders as parameter
with declaration; local references as parameter; other symbols by the kind of their
workspace definition; qualifiers as namespace. A string whose preceding prefix token is one
of the dialect's declared regexp string prefixes SHALL be typed regexp. Tokens spanning
lines SHALL be split per line.

#### Scenario: Classification
- **WHEN** tokens are requested for `(defun f (x) (car x)) ; c`
- **THEN** `defun` is keyword, `f` is function+definition, first `x` is parameter+declaration, `car` is function+defaultLibrary, second `x` is parameter, `; c` is comment

#### Scenario: Regexp string
- **WHEN** tokens are requested for `#rx"a+"` in `common-lisp`
- **THEN** the string token is typed regexp
