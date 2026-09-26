# assist Specification

## Purpose
Help while typing: complete names, show call signatures and explain symbols, using only
static knowledge from the workspace and dialect data.

## Requirements

### Requirement: Completion sources
Completion SHALL offer, for the symbol prefix before the cursor, locals visible at the cursor,
workspace definitions from same-dialect files, and (when `completion.builtins` is true) the
dialect's special forms and builtins; each label appears once, locals first on equal match
quality, ranked by fuzzy match quality, at most `completion.max_items` items with the list
marked incomplete when truncated. Each item replaces the prefix.

#### Scenario: Local and workspace
- **WHEN** completing `he` inside `(let ((hello 1)) (he))` in a workspace defining `helper`
- **THEN** items include `hello` (variable) before `helper` (function)

#### Scenario: No completion in strings and comments
- **WHEN** completing inside `"he"` or `; he`
- **THEN** the result is empty

### Requirement: Qualified completion
When the prefix is qualified (`ns/x`, `pkg:x`), completion SHALL offer only workspace
definitions in the qualifier's namespace (after aliases) and replace only the base name.

#### Scenario: Alias prefix
- **WHEN** `(ns c (:require [app.util :as u]))` and completing `u/fo` with `app.util` defining `format-x`
- **THEN** `format-x` is offered with an edit covering `fo`

### Requirement: Signature help
Inside a call, signature help SHALL show every signature of the called definition as
`(name params...)`, choose the arity that fits the argument count, and mark the active
parameter by argument position, mapping arguments past `&rest`, `&body`, `&` or `.` to the
rest parameter and skipping lambda-list markers.

#### Scenario: Rest parameter
- **WHEN** the cursor is on the third argument of `(f 1 2 3)` for `(defun f (a &rest more))`
- **THEN** the active parameter is `more`

#### Scenario: Arity selection
- **WHEN** the cursor is on the second argument of `(g 1 2)` for `(defn g ([x] x) ([x y] y))`
- **THEN** the active signature is `(g x y)` with active parameter `y`

### Requirement: Hover
Hover SHALL describe the symbol under the cursor: for workspace definitions their
signature, kind, namespace, docstring and file; for locals, that they are local bindings;
for dialect special forms, builtins and constants, their category.

#### Scenario: Documented function
- **WHEN** hovering `area` in a call where `(defun area (w h) "Area." ...)` is defined
- **THEN** the hover contains `(area w h)` and `Area.`
