# analysis Specification

## Purpose
Determine statically what each symbol in a file means: definitions, local bindings with
shadowing, namespaces and aliases, without evaluating code.

## Requirements

### Requirement: Definitions from dialect data
The analyzer SHALL extract definitions from forms listed in the dialect's `defs` table, and
from forms whose head starts with a configured `def_prefixes` entry and whose name position
holds a symbol, recording name, kind, form and name ranges, parameter lists, docstring and
enclosing namespace.

#### Scenario: Common Lisp function
- **WHEN** analyzing `(defun area (w h) "Area." (* w h))`
- **THEN** a function `area` is defined with parameters `(w h)` and docstring `Area.`

#### Scenario: Clojure multi-arity
- **WHEN** analyzing `(defn f "doc" ([x] x) ([x y] y))`
- **THEN** `f` has two signatures `[x]` and `[x y]` and docstring `doc`

#### Scenario: Scheme curried define
- **WHEN** analyzing `(define (sq x) (* x x))` in `scheme`
- **THEN** a function `sq` is defined with parameter `x`

#### Scenario: Setf function name
- **WHEN** analyzing `(defun (setf foo) (v x) v)` in `common-lisp`
- **THEN** a function named `(setf foo)` is defined

### Requirement: Namespaces
Definitions SHALL carry the namespace set by the dialect's namespace forms (`in-package`,
`ns`) that precede them; Clojure `:as` aliases in `ns` forms SHALL map to their namespace.

#### Scenario: In-package
- **WHEN** `(in-package :app)` precedes `(defun run ())`
- **THEN** `run` has namespace `app`

#### Scenario: Alias
- **WHEN** `(ns a (:require [clojure.string :as str]))` is analyzed
- **THEN** alias `str` maps to `clojure.string`

### Requirement: Local bindings
The analyzer SHALL create binders for the dialect's binding forms and definition parameters,
visible from the end of the binding (or parameter list) to the end of the form, and SHALL
resolve each symbol occurrence to the innermost visible binder of the same name, otherwise
to a global name.

#### Scenario: Shadowing
- **WHEN** analyzing `(let ((x 1)) (let ((x 2)) x) x)`
- **THEN** the first `x` in body position resolves to the inner binder and the last to the outer binder

#### Scenario: Init does not see its own binder
- **WHEN** analyzing `(let ((x (f x))) x)`
- **THEN** the `x` inside `(f x)` resolves globally

#### Scenario: Destructuring
- **WHEN** analyzing `(let [{:keys [a b] :as m} v] (+ a b))` in `clojure`
- **THEN** `a`, `b` and `m` are binders and `a`, `b` in the body resolve to them

### Requirement: Names and case
Occurrences SHALL be compared by their normalized base name: qualifiers (`pkg:x`, `ns/x`) are
split off, and names are case-folded in case-insensitive dialects. Keywords, numbers,
constants, strings, characters and datum-commented forms SHALL NOT be occurrences.

#### Scenario: Case-insensitive
- **WHEN** Common Lisp code defines `foo` and calls `(FOO)`
- **THEN** the call is an occurrence of `foo`

### Requirement: Declared indentation
The analyzer SHALL record indentation hints declared as `(declare (indent N))` in a definition
body and `:style/indent N` in Clojure metadata or attribute maps.

#### Scenario: Emacs Lisp declare
- **WHEN** analyzing `(defmacro with-x (a &rest body) (declare (indent 1)) ...)`
- **THEN** `with-x` has indentation hint 1
