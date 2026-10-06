# Tasks

## 1. Dialect fields

- [x] 1.1 `rest_markers` (default `["&rest", "&body", "&", ".", "&more"]`) and
      `key_markers` (default `["&key"]`) used by `param_slots`; verify
      `rest_markers_are_dialect_data` (custom marker honored, stock marker replaced,
      stock dialect unchanged) and the signature-help tests stay green
- [x] 1.2 `indent_declarations` list (`{ head, name }` list shapes, `{ attribute }` map
      keys; default: `(declare (indent N))` and `:style/indent N`) used by `indent_hint`;
      verify `indent_declarations_are_dialect_data` (custom form honored, replaced stock
      form ignored) and the declared-indent tests in `src/analysis/tests.rs` and
      `tests/formatting.rs` stay green
- [x] 1.3 `regexp_string_prefixes` (default `["#", "#rx", "#px"]`) used by semantic-token
      classification; verify `signature_conventions_are_dialect_data` (custom prefix
      accepted, stock prefix rejected) and the semantic-token tests stay green

## 2. Shared cell predicates

- [x] 2.1 `under_function_ref`, `function_form_argument`, `in_cond_clause` and
      `in_reader_vector` extracted as shared tree-level functions used by both
      `Walker::function_cell` and the completion gap logic; verify the cell tests in
      `src/analysis/tests.rs`, `tests/navigation.rs` and `tests/diagnostics.rs` stay green

## 3. Docs

- [x] 3.1 New dialect keys documented in the README dialect key list; verify `task spec`
      passes and `llsp dialects` is unchanged (`tests/cli.rs::dialects_listed`)
