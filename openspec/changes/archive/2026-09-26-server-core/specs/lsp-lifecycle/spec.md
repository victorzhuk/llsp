# Spec Delta

## Purpose

Define how llsp speaks the Language Server Protocol: transports, lifecycle, error codes and
position encoding negotiation.

## ADDED Requirements

### Requirement: Transports
The server SHALL speak LSP over stdio by default and over TCP only when explicitly
requested, binding to a loopback address only.

#### Scenario: Non-loopback TCP address
- **WHEN** the server is started with `--listen 0.0.0.0:9000`
- **THEN** it exits with an error stating only loopback addresses are allowed

### Requirement: Lifecycle
The server SHALL answer `initialize` with its capabilities and server info, accept
`initialized`, answer `shutdown` with null, and exit on `exit`, with exit code 0 only if
`shutdown` was received first.

#### Scenario: Clean shutdown
- **WHEN** the client sends `shutdown` then `exit`
- **THEN** the server replies null to shutdown and terminates with code 0

### Requirement: Unknown methods
The server SHALL answer unknown requests with error `MethodNotFound` (-32601) and ignore
unknown notifications.

#### Scenario: Unknown request
- **WHEN** the client sends request `foo/bar`
- **THEN** the response carries error code -32601

### Requirement: Position encoding
The server SHALL use UTF-8 positions when the client lists `utf-8` in
`general.positionEncodings`, otherwise UTF-16, and SHALL report the choice in
`capabilities.positionEncoding`.

#### Scenario: UTF-16 client
- **WHEN** a client without position encodings opens `(ö x)` and asks about `x`
- **THEN** positions count `ö` as one UTF-16 unit
