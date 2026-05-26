# cluster-grain-runtime-operational-contract Specification

## Purpose
Cluster Grain runtime の identity resolution、topology invalidation、passivation、rolling update 境界を定義する。
## Requirements
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

### Requirement: Topology changes invalidate absent authorities

Cluster topology updates SHALL invalidate activation and PID cache entries that belong to authorities no longer present in the active authority set. Lookup MUST NOT return a PID for an authority that is absent from the latest topology.

Leave or down signals MUST invalidate only activations and PID cache entries owned by the matching authority. Activations owned by authorities that remain in the active topology MUST stay reusable.

#### Scenario: topology replacement removes stale authority cache

- **GIVEN** a `GrainKey` resolved to an authority in the previous topology
- **WHEN** topology is replaced with a set that does not include that authority
- **THEN** activation and PID cache entries for the absent authority are invalidated
- **AND** the next resolution uses only authorities from the new topology

#### Scenario: member departure invalidates matching authority

- **GIVEN** a `GrainKey` has an activation owned by an authority
- **WHEN** the identity lookup observes that authority leaving or being downed
- **THEN** activation and PID cache entries for that authority are invalidated
- **AND** a later resolution MUST NOT return the previous PID

#### Scenario: unknown member departure is a no-op

- **GIVEN** activation and PID cache entries belong to active authorities
- **WHEN** identity lookup observes departure for an unknown authority
- **THEN** existing activation and PID cache entries remain available
- **AND** subsequent lookup for the same `GrainKey` may still return the active cached PID

#### Scenario: leave invalidates only matching authority ownership

- **GIVEN** multiple active activations are owned by different authorities
- **WHEN** one authority leaves or is downed
- **THEN** activations and PID cache entries owned by that authority are invalidated
- **AND** activations owned by remaining authorities stay reusable

### Requirement: Passivation removes reusable activation state

Passivation SHALL remove idle activation state and its PID cache entry. A passivated `GrainKey` MUST be resolved as a new placement decision instead of a cache hit.

#### Scenario: idle activation is passivated

- **GIVEN** a `GrainKey` has an activation whose last access is older than the idle TTL
- **WHEN** passivation is evaluated at the current time
- **THEN** the activation is removed
- **AND** the PID cache entry for that `GrainKey` is invalidated
- **AND** a passivation event is observable

#### Scenario: recent activation is retained

- **GIVEN** a `GrainKey` has an activation whose last access is within the idle TTL
- **WHEN** passivation is evaluated at the current time
- **THEN** the activation remains reusable
- **AND** lookup may return the active cached PID

### Requirement: Rolling update contract is bounded to stale placement prevention

During rolling update, Grain runtime SHALL prevent stale placement reuse for departed authorities and SHALL resolve against the latest topology. It MUST NOT promise shard rebalance, remembered entity recovery, or in-flight request draining as part of this contract.

#### Scenario: replacement node becomes the only resolution candidate

- **GIVEN** an old authority owns a `GrainKey`
- **AND** a replacement authority joins the topology
- **WHEN** the old authority leaves or is downed and topology is updated
- **THEN** stale activation and PID cache entries for the old authority are invalidated
- **AND** the next resolution selects from the replacement topology

#### Scenario: rolling update does not imply rebalance guarantee

- **WHEN** topology changes during rolling update
- **THEN** Grain runtime guarantees stale authority invalidation and re-resolution
- **AND** Grain runtime does not guarantee minimum movement, remembered activation recovery, or automatic shard draining in this capability

### Requirement: Provider and downing integration remains an input boundary

Cluster providers, failure detectors, and downing strategies SHALL feed topology update or member departure signals into the Grain runtime contract. This capability MUST NOT require a specific discovery backend, Split Brain Resolver, or reachability matrix implementation.

#### Scenario: provider supplies topology update

- **WHEN** a provider publishes or applies a topology update to identity lookup
- **THEN** Grain runtime applies the topology invalidation contract
- **AND** provider-specific discovery details remain outside this capability

#### Scenario: downing supplies member departure

- **WHEN** failure detection or downing decides that an authority has departed
- **THEN** Grain runtime handles it as member departure input
- **AND** downing decision rules remain outside this capability
