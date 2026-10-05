## MODIFIED Requirements

### Requirement: Transports
The server SHALL speak LSP over stdio by default and over TCP only when explicitly
requested, binding to a loopback address only. The TCP listener SHALL accept exactly one
unauthenticated connection: any local process that connects first owns the session, and the
server SHALL NOT speak LSP to a second connection.

#### Scenario: Non-loopback TCP address
- **WHEN** the server is started with `--listen 0.0.0.0:9000`
- **THEN** it exits with an error stating only loopback addresses are allowed

#### Scenario: Second connection
- **WHEN** a second local process connects to the listening port while a session is active
- **THEN** the first session keeps answering and the second connection receives no LSP traffic
