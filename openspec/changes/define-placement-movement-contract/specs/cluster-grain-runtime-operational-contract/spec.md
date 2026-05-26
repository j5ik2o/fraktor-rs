## ADDED Requirements

### Requirement: Placement movement is bounded by active topology changes

Grain runtime SHALL use the active authority topology when making new placement decisions. It MUST NOT move existing active activations only because a new authority joined. It MUST invalidate activations and PID cache entries owned by authorities that leave or are downed.

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

#### Scenario: leave invalidates only matching authority ownership

- **GIVEN** multiple active activations are owned by different authorities
- **WHEN** one authority leaves or is downed
- **THEN** activations and PID cache entries owned by that authority are invalidated
- **AND** activations owned by remaining authorities stay reusable

### Requirement: Rendezvous placement remains deterministic without rebalance semantics

Rendezvous hashing SHALL provide deterministic owner selection for the same `GrainKey` and same authority topology. This capability MUST NOT guarantee minimum movement, proactive rebalance, remembered entity recovery, or in-flight request draining.

#### Scenario: same topology produces stable owner

- **GIVEN** the same non-empty authority topology
- **WHEN** the same `GrainKey` is selected multiple times
- **THEN** Rendezvous placement returns the same authority each time

#### Scenario: topology change permits owner change without movement guarantee

- **WHEN** authority topology changes by join or leave
- **THEN** future placement decisions may select a different owner according to Rendezvous hashing
- **AND** the runtime does not guarantee minimum movement across all keys

#### Scenario: rolling update is stale-placement prevention only

- **WHEN** rolling update changes the authority topology
- **THEN** Grain runtime prevents reuse of activations owned by departed authorities
- **AND** Grain runtime does not guarantee automatic shard rebalance, remembered entity recovery, or request draining
