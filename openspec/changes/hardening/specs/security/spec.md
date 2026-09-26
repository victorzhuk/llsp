# Spec Delta

## Purpose

State and enforce what llsp will never do with the code it reads, and how it bounds
resource use on hostile or oversized input.

## ADDED Requirements

### Requirement: No code execution
The server MUST NOT evaluate, compile, load or expand user code, spawn processes, or open
network connections other than the explicitly requested loopback LSP listener.

#### Scenario: Read-time evaluation in a document
- **WHEN** a document containing `#.(run-program "rm")` is opened and every feature is requested on it
- **THEN** no process is started and responses are computed from syntax only

### Requirement: Oversized documents
Open documents larger than `files.max_file_size` SHALL be kept for synchronization but not
analyzed: diagnostics are empty, feature requests return empty results, and one warning
message is shown per document.

#### Scenario: Huge file
- **WHEN** `files.max_file_size = 100` and a 1 KB document is opened
- **THEN** its diagnostics are empty, document symbols are empty, and a warning mentions `files.max_file_size`

### Requirement: Robustness
For any document text in any dialect and any position, every supported request SHALL return
a result or a protocol error other than an internal error, and the server SHALL keep running.

#### Scenario: Random input
- **WHEN** random text is opened and each feature is requested at random positions
- **THEN** every response is a result and the server still answers `shutdown`

### Requirement: Log hygiene
Logs SHALL NOT contain document text, and log files SHALL be created readable only by
their owner.

#### Scenario: Log file mode
- **WHEN** the server starts with `--log-file` on a Unix system
- **THEN** the file is created with mode 0600
