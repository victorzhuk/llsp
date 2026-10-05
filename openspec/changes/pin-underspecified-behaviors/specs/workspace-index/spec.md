## ADDED Requirements

### Requirement: Fixed roots
The server SHALL determine its workspace roots at initialization and SHALL NOT rescan when
they change; `workspace/didChangeWorkspaceFolders` SHALL be ignored.

#### Scenario: Folder added later
- **WHEN** a client sends `workspace/didChangeWorkspaceFolders` adding another folder
- **THEN** the server keeps running, answers requests for the original roots, and does not
  index the new folder

### Requirement: Watcher registration
When the client advertises dynamic registration for watched files and `workspace.index` is
on, the server SHALL register a `didChangeWatchedFiles` watcher whose pattern covers every
dialect's file extensions; otherwise it SHALL register nothing and rely on the client.

#### Scenario: Dynamic registration
- **WHEN** a client initializes with `workspace.didChangeWatchedFiles.dynamicRegistration = true`
- **THEN** the server sends `client/registerCapability` for `workspace/didChangeWatchedFiles`
  with a glob covering the dialect extensions

## MODIFIED Requirements

### Requirement: Confinement
The index MUST NOT read files outside the workspace roots: symbolic links are not followed,
and watched-file events for paths outside the roots are ignored. Confinement SHALL be
enforced when a file is read: the canonicalized path SHALL be re-checked against the roots
immediately before every read, not only at discovery.

#### Scenario: Symlink out of root
- **WHEN** the root contains a symlink to a directory outside it
- **THEN** files behind the symlink are not indexed

#### Scenario: Component swapped during scan
- **WHEN** a directory discovered inside a root is replaced by a symlink to outside before
  its files are read
- **THEN** those files are not read
