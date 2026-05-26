## MODIFIED Requirements

### Requirement: Grain identity resolution is deterministic and state-aware

Grain identity lookup SHALL resolve a `GrainKey` from the current authority topology and SHALL report unresolved states explicitly. It MUST NOT return a stale PID when the lookup is stopped, has no authorities, or is waiting for asynchronous activation.

Rendezvous hashing SHALL provide deterministic owner selection for the same `GrainKey` and same authority topology. New placement decisions MUST use the active authority topology. Existing active activations MUST NOT move only because a new authority joined.

This requirement SHALL be treated as the bounded Placement scalability contract for the current Grain runtime roadmap slice. It MUST NOT imply least-shard rebalance, minimum movement guarantees, remembered entity recovery, persistence-backed activation recovery, or in-flight request draining.

#### Scenario: no authority is reported explicitly

- **WHEN** member-mode identity lookup resolves a `GrainKey` before any authority is present
- **THEN** resolution fails with `LookupError::NoAuthority`
- **AND** no PID is cached for that `GrainKey`

#### Scenario: same key and topology resolve deterministically

- **GIVEN** two identity lookup instances have the same authority topology
- **WHEN** both instances resolve the same `GrainKey`
- **THEN** both select the same authority
- **AND** both produce a placement decision for that authority

#### Scenario: cache hit reuses the active PID

- **GIVEN** a `GrainKey` has been resolved and its activation is still valid
- **WHEN** the same `GrainKey` is resolved again before PID TTL or passivation invalidates it
- **THEN** lookup returns the same PID
- **AND** the returned PID belongs to the same authority decision

#### Scenario: distributed activation exposes pending resolution

- **GIVEN** distributed activation is enabled for a local authority
- **WHEN** lookup starts resolving a `GrainKey` that requires lock, load, ensure, and store commands
- **THEN** lookup reports `LookupError::Pending` until the command results complete
- **AND** lookup returns a PID only after activation is stored and the lock is released

#### Scenario: same topology produces stable owner

- **GIVEN** the same non-empty authority topology
- **WHEN** the same `GrainKey` is selected multiple times
- **THEN** Rendezvous placement returns the same authority each time

#### Scenario: join does not move existing active activation

- **GIVEN** a `GrainKey` has an active activation owned by an authority in the current topology
- **WHEN** a new authority joins the topology
- **THEN** lookup may continue returning the existing active PID for that `GrainKey`
- **AND** no passivation or PID cache drop is emitted only because of the join

#### Scenario: new resolution after join uses expanded topology

- **GIVEN** a new authority joined the topology
- **WHEN** a `GrainKey` without an active activation is resolved
- **THEN** placement selection uses the expanded active topology
- **AND** the selected authority belongs to that topology

#### Scenario: topology change permits owner change without movement guarantee

- **WHEN** authority topology changes by join or leave
- **THEN** future placement decisions may select a different owner according to Rendezvous hashing
- **AND** the runtime does not guarantee minimum movement across all keys
