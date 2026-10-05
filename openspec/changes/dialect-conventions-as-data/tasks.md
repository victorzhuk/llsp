# Tasks

## 1. Dialect fields

- [ ] 1.1 Add the rest-marker field (default `["&rest", "&body", "&", ".", "&more"]` plus
      `&key` handling) and use it in `param_slots` (`src/features/assist.rs:457-472`); verify
      signature-help tests stay green and a custom-dialect test maps arguments past a
      user-declared marker to its rest parameter
- [ ] 1.2 Add the indent-declaration field describing which forms declare indentation
      (default: `(declare (indent N))` lists and `:style/indent N` attribute maps) and use it
      in `indent_hint` (`src/analysis.rs:404-439`); verify the declared-indent tests in
      `src/analysis/tests.rs` and `tests/formatting.rs` stay green and a custom-dialect test
      records a hint from its own declaration form
- [ ] 1.3 Add the regexp-prefix field (default `["#", "#rx", "#px"]`) and use it in
      semantic-token classification (`src/features/structure.rs:231-236`); verify the
      semantic-token tests stay green and a custom-dialect test types only its declared
      prefix's strings as regexp

## 2. Shared cell predicates

- [ ] 2.1 Extract the function-reference, cond-clause-test and reader-vector predicates into
      shared tree-level functions used by both `Walker::function_cell`
      (`src/analysis.rs:694-730`) and `completion_cell`/`gap_value_cell`
      (`src/features/assist.rs:338-387`); verify the cell tests in `src/analysis/tests.rs`,
      `tests/navigation.rs` and `tests/diagnostics.rs` stay green

## 3. Docs

- [ ] 3.1 Document the new dialect keys in the README dialect key list; verify `task spec`
      and `llsp dialects` unchanged
