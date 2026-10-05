## ADDED Requirements

### Requirement: Bounded dialect definitions
Loading dialect definitions SHALL bound the `extends` chain depth and the total number of
dialect definitions. Exceeding a bound SHALL fail with an error naming the bound and the
offending dialect; the server SHALL NOT recurse unboundedly or clone tables per chain level.

#### Scenario: Deep chain
- **WHEN** config declares 40 dialects each extending the next
- **THEN** loading fails with an error naming the depth bound, and nothing aborts

#### Scenario: Too many dialects
- **WHEN** config declares 5000 dialects each extending the next
- **THEN** loading fails with an error naming the count bound, and nothing aborts

#### Scenario: Stock dialects
- **WHEN** no custom dialects are declared
- **THEN** all nine built-in dialects load unchanged

### Requirement: Bounded numeric settings
Numeric settings SHALL have documented upper bounds. A value beyond its bound SHALL be
rejected by validation with an error naming the key and the bound, exactly like a wrong
value type. Formatter indent targets SHALL additionally be clamped to the validated maximum
so nested documents cannot produce unbounded edit text.

#### Scenario: Overflowing debounce
- **WHEN** `diagnostics.debounce_ms = 18446744073709551615` is set
- **THEN** validation fails naming `debounce_ms`, and the server does not panic on the next edit

#### Scenario: Huge body indent
- **WHEN** `format.body_indent = 4000000000` is set
- **THEN** validation fails naming `body_indent`
