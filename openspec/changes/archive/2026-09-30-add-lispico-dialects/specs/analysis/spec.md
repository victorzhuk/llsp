## ADDED Requirements

### Requirement: Cell-aware resolution
In a dialect with `function_cells = true`, an occurrence in call-head position or as the
argument of `function` or `#'` SHALL resolve only to function-cell bindings, and any other
occurrence only to value-cell bindings. The same rule SHALL apply to definitions found in other
files of the same dialect in the workspace index, not only to definitions in the current
document.

#### Scenario: Call head skips value binder
- **WHEN** analyzing `(defun f (x) x) (let ((f 1)) (f f))` in `lispico-cl`
- **THEN** the head `f` resolves globally to `defun f` and the argument `f` to the `let` binder

#### Scenario: Function reference
- **WHEN** analyzing `(mapcar #'g xs)` and `(funcall (function g) 1)` with `(defun g (x) x)` in `lispico-cl`
- **THEN** both `g` occurrences resolve to `defun g`

#### Scenario: Same name in both cells
- **WHEN** a `lispico-cl` file contains `(def n 1)` and `(defun n () 2)`
- **THEN** `(n)` navigates to `defun n`, a value-position `n` navigates to `def n`, and neither is a duplicate definition

#### Scenario: Lisp-1 unchanged
- **WHEN** analyzing `(let [f inc] (f 1))` in `lispico-clojure`
- **THEN** the head `f` resolves to the `let` binder

#### Scenario: Cross-file cells
- **WHEN** a `lispico-cl` file containing `(n)` is analyzed while `(defun n () 2)` lives in
  another file of the same dialect in the workspace, and a value-position `n` while `(def n 1)`
  lives in a third
- **THEN** the call head navigates to the `defun` and the value occurrence to the `def`
