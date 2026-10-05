## MODIFIED Requirements

### Requirement: Robustness
For any document text in any dialect and any position, every supported request SHALL return
a result or a protocol error other than an internal error, and the server SHALL keep running.
For any input that passes validation — documents, configurations and watched-file events —
resource use SHALL stay bounded: nothing SHALL abort the process, and output SHALL stay
proportional to the input.

#### Scenario: Random input
- **WHEN** random text is opened and each feature is requested at random positions
- **THEN** every response is a result and the server still answers `shutdown`

#### Scenario: Hostile configuration
- **WHEN** a project `.llsp.toml` declares a 5000-dialect `extends` chain or an overflowing
  numeric value
- **THEN** the server reports a validation error naming the key, and keeps running

#### Scenario: Adversarial formatting
- **WHEN** a 10 000-line document of `(`-per-line lines is formatted with valid configuration
- **THEN** formatting completes, with every line's indentation clamped to the validated maximum

### Requirement: Log hygiene
Logs SHALL NOT contain document text, and log files SHALL be created readable only by their
owner and SHALL stay that way: an existing log file's mode SHALL be normalized to 0600, and
a path behind a symbolic link SHALL NOT be written through.

#### Scenario: Log file mode
- **WHEN** the server starts with `--log-file` on a Unix system
- **THEN** the file is created with mode 0600

#### Scenario: Pre-existing log file
- **WHEN** the log path already exists with mode 0644
- **THEN** the mode is normalized to 0600 while the server logs to it

#### Scenario: Project file sets logging
- **WHEN** the workspace `.llsp.toml` contains a `[log]` section
- **THEN** it is ignored, and logging is configured only by the user file, environment, command line and editor
