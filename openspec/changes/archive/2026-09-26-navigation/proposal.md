# Proposal

## Why

With analysis and the index in place, editors can navigate code: jump to definitions, list
references and symbols, and rename safely across the workspace.

## What Changes

- `textDocument/documentSymbol` (hierarchical), `workspace/symbol` (fuzzy).
- `textDocument/definition`, `textDocument/references`, `textDocument/documentHighlight`.
- `textDocument/prepareRename` and `textDocument/rename` for locals and workspace names.
- Requests with undecodable params answer `InvalidParams`.
- New setting `workspace.max_symbols`.

## Capabilities

### New Capabilities

- `navigation`: symbol listing, definition, references, highlights and rename.

### Modified Capabilities

- `lsp-lifecycle`: invalid parameter handling.
- `configuration`: `workspace.max_symbols`.

## Impact

New `src/features/` modules; server dispatch and capabilities.
