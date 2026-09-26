# Spec Delta

## ADDED Requirements

### Requirement: Clause binding shape
The `clauses` binding shape SHALL treat each form after the head as `(params body...)`,
binding the params within that clause.

#### Scenario: case-lambda
- **WHEN** analyzing `(case-lambda ((x) x) ((x y) y))` in `scheme`
- **THEN** each clause binds its own parameters

### Requirement: List parameter search
Definition specs SHALL accept `params = "list"`, meaning the first parenthesized list after
the name holds the parameters.

#### Scenario: defmethod qualifiers
- **WHEN** analyzing `(defmethod area :around ((s square)) (side s))` in `common-lisp`
- **THEN** `s` is a parameter binder and `side s` refers to it

### Requirement: Iterator binding shape
The `iterator` binding shape SHALL bind every element of the bracketed form at position 1
except the last, which is the iterated expression.

#### Scenario: Fennel each
- **WHEN** analyzing `(each [k v (pairs t)] (print k v))` in `fennel`
- **THEN** `k` and `v` are binders and `pairs`, `t` are not
