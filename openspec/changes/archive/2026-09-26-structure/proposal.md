# Proposal

## Why

Few editors ship good highlighting or structural navigation for every Lisp dialect. The
syntax tree and analysis already know forms, comments, strings, definitions and bindings, so
the server can provide folding, expand-selection and semantic highlighting uniformly.

## What Changes

- `textDocument/foldingRange`: multi-line forms, block comments, runs of line comments.
- `textDocument/selectionRange`: atom → enclosing forms → top-level form.
- `textDocument/semanticTokens/full` and `/range`: comments, strings, characters, numbers,
  keywords, special forms, builtins, constants, definitions, parameters, locals, namespaces,
  macros and functions, with declaration/definition/defaultLibrary modifiers.

## Capabilities

### New Capabilities

- `structure`: folding, selection ranges and semantic tokens.

### Modified Capabilities

None.

## Impact

New `src/features/structure.rs`; capabilities advertise the semantic token legend.
