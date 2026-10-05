## MODIFIED Requirements

### Requirement: Live updates
Open documents SHALL override their on-disk version in the index on every change; closing a
document SHALL restore the on-disk version; watched-file create/change/delete events SHALL
update the index for files that are not open. Index updates from watched-file events and
document close SHALL be applied without blocking request handling, like the initial scan.

#### Scenario: Unsaved definition
- **WHEN** an open document gains `(defun fresh ())` without saving
- **THEN** the index lists `fresh` for that document

#### Scenario: Burst of events
- **WHEN** a branch switch fires a batch of watched-file events and a feature request
  arrives while the batch is being processed
- **THEN** the request is answered from the index state before or after the batch, without
  waiting for the whole batch
