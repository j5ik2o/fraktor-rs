# cluster-grain-runtime-contract-tests Specification

## Purpose
Defines executable cluster-core contract-test coverage for Grain runtime operational behavior.
## Requirements
### Requirement: Pending distributed activation is covered by contract tests

The cluster-core test suite SHALL cover distributed Grain activation through the public identity lookup surface. A `PartitionIdentityLookup` configured for distributed activation MUST report `LookupError::Pending` from `IdentityLookup::resolve` while placement command results are outstanding, and MUST return the stored PID only after the placement command sequence completes.

#### Scenario: first public resolve reports pending

- **GIVEN** `PartitionIdentityLookup` is in member mode with distributed activation enabled
- **AND** the active topology contains a local authority for the requested `GrainKey`
- **WHEN** `IdentityLookup::resolve` is called before placement command results complete
- **THEN** the call returns `LookupError::Pending`
- **AND** no completed PID is returned

#### Scenario: repeated public resolve stays pending before completion

- **GIVEN** distributed activation has started for a `GrainKey`
- **AND** the emitted placement command sequence has not completed
- **WHEN** `IdentityLookup::resolve` is called again for the same `GrainKey`
- **THEN** the call returns `LookupError::Pending`
- **AND** it does not return a stale PID or a fabricated activation result

#### Scenario: public resolve returns stored PID after completion

- **GIVEN** distributed activation has started for a `GrainKey`
- **AND** the placement command sequence completes with a stored activation record
- **WHEN** `IdentityLookup::resolve` is called again for the same `GrainKey`
- **THEN** the call returns the stored PID
- **AND** the placement decision still belongs to the selected authority
