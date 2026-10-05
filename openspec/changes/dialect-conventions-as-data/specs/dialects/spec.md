## ADDED Requirements

### Requirement: Call and declaration conventions as data
A dialect definition SHALL declare the conventions that signature help, indentation hints
and highlighting need: the rest-parameter markers of its lambda lists, the forms and
attribute keys that declare indentation, and the string prefixes whose strings are regexp
literals. These SHALL be read from dialect data, with the stock dialects' current values as
defaults when omitted.

#### Scenario: Custom rest marker
- **WHEN** dialect `mine` extends `common-lisp` with rest marker `&others` and signature
  help is requested on the third argument of `(f 1 2 3)` for `(defun f (a &others r) r)`
- **THEN** the active parameter is `r`

#### Scenario: Custom indent declaration
- **WHEN** dialect `mine` declares its own indent declaration form and a definition carries
  it with value 1
- **THEN** the definition records indentation hint 1 and formatting of its uses follows it

#### Scenario: Regexp prefix override
- **WHEN** dialect `mine` declares `#re` as its regexp string prefix
- **THEN** semantic tokens type a string after `#re` as regexp, and a string after `#rx` as
  string

#### Scenario: Stock dialects unchanged
- **WHEN** a stock dialect omits the new fields
- **THEN** signature help, indentation hints and semantic tokens behave exactly as before
