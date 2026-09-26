# Spec Delta

## Purpose

Maintain cross-file knowledge of definitions and references for the whole workspace, safely
and within resource limits.

## ADDED Requirements

### Requirement: Initial scan
After initialization the server SHALL index files under the workspace roots whose dialect is
known by extension, honoring `.gitignore`, `workspace.exclude` globs, `workspace.max_files`
and `files.max_file_size`, without blocking request handling. `workspace.index = false`
SHALL disable scanning.

#### Scenario: Excluded directory
- **WHEN** the root contains `target/gen.lisp` and `src/a.lisp`
- **THEN** only `src/a.lisp` is indexed

#### Scenario: File limit
- **WHEN** the root holds more lisp files than `workspace.max_files`
- **THEN** exactly `max_files` files are indexed and a warning is logged

### Requirement: Confinement
The index MUST NOT read files outside the workspace roots: symbolic links are not followed,
and watched-file events for paths outside the roots are ignored.

#### Scenario: Symlink out of root
- **WHEN** the root contains a symlink to a directory outside it
- **THEN** files behind the symlink are not indexed

### Requirement: Live updates
Open documents SHALL override their on-disk version in the index on every change; closing a
document SHALL restore the on-disk version; watched-file create/change/delete events SHALL
update the index for files that are not open.

#### Scenario: Unsaved definition
- **WHEN** an open document gains `(defun fresh ())` without saving
- **THEN** the index lists `fresh` for that document
