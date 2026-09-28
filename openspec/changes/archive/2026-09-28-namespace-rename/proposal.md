# Proposal

## Why

Rename and find-references matched workspace names by spelling alone. Renaming `run` in
`app.a` also rewrote `app.b/run` and calls imported from `app.b`, which breaks code.

## What Changes

- Each global reference resolves to a namespace: its qualifier (through aliases) or `:refer`,
  else the current namespace when it defines the name, else the only namespace that does.
- Rename edits only references resolving to the target's namespace; prepare-rename refuses a
  reference that is ambiguous between namespaces.
- References return occurrences resolving to the same namespace; an ambiguous reference
  still lists every same-named occurrence.
- Dialects without namespaces behave as before.

## Capabilities

### Modified Capabilities

- `navigation`: namespace-aware references and rename.

## Impact

Analysis records namespace switch points and `:refer` lists; the index keeps each reference's
explicit namespace.
