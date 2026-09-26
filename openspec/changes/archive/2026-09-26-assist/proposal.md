# Proposal

## Why

Editing Lisp is faster when the editor suggests names, shows parameters while typing a call,
and explains a symbol on hover. The index and analysis already hold everything needed.

## What Changes

- `textDocument/completion`: visible locals, workspace definitions, dialect special forms
  and builtins; fuzzy ranking; qualified prefixes (`str/jo`, `pkg:fo`) filter by namespace.
- `textDocument/signatureHelp`: signatures of the called definition (all arities), active
  parameter aware of lambda-list markers (`&optional`, `&rest`, `&`).
- `textDocument/hover`: signature, kind, namespace, docstring and location; special forms,
  builtins and locals are labeled.
- Settings `completion.max_items` and `completion.builtins`.

## Capabilities

### New Capabilities

- `assist`: completion, signature help and hover.

### Modified Capabilities

- `configuration`: completion settings.

## Impact

New `src/features/assist.rs`; capabilities advertised in `initialize`.
