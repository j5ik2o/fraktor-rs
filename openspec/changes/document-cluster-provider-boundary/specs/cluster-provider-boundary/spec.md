## ADDED Requirements

### Requirement: Provider inputs are normalized before Grain runtime use

Cluster providers SHALL translate discovery, seed, lifecycle, or explicit membership operations into cluster topology input before Grain runtime identity and placement logic observes them. Cluster core MUST NOT branch on provider-specific discovery details when applying Grain runtime topology invalidation.

#### Scenario: topology update is provider-neutral

- **WHEN** a provider publishes `ClusterEvent::TopologyUpdated`
- **THEN** cluster core treats the update as topology input independent of provider type
- **AND** Grain runtime invalidation rules operate on authorities from that update

#### Scenario: discovery details remain outside core placement

- **WHEN** topology input originates from static configuration, remoting lifecycle, or AWS ECS task discovery
- **THEN** identity and placement logic use the resulting authority set
- **AND** identity and placement logic do not inspect the original discovery backend

### Requirement: Local and static providers expose bounded membership behavior

Local and static providers SHALL define their membership behavior at the provider boundary. Local provider MUST publish topology updates for explicit join, leave, and down operations that change membership. Static provider MUST only publish its configured static topology on start and MUST NOT perform discovery.

#### Scenario: local explicit join publishes joined topology

- **WHEN** local provider receives an explicit join for a non-member authority
- **THEN** it publishes a topology update with that authority in `joined`
- **AND** the authority becomes part of the provider's current member set

#### Scenario: local explicit leave or down publishes left topology

- **WHEN** local provider receives explicit leave or down for a current member authority
- **THEN** it publishes a topology update with that authority in `left`
- **AND** the authority is removed from the provider's current member set

#### Scenario: static provider publishes configured topology on start

- **WHEN** static provider starts with a configured topology
- **THEN** it publishes that topology as a cluster topology update
- **AND** it does not start a discovery subscription or polling task

### Requirement: Core-defined ports are implemented by std adapters

Cluster core SHALL define provider ports and lifecycle policy, while std adapters SHALL implement those ports for std-specific lifecycle and discovery sources. Std adapters MUST NOT own Grain runtime policy. Remoting lifecycle subscription MUST be controlled by the returned subscription lifetime and MUST NOT keep the local provider alive through a strong reference. AWS ECS discovery polling MUST be owned by the AWS ECS provider start/shutdown lifecycle.

#### Scenario: remoting adapter supplies provider port input

- **WHEN** std adapter subscribes a local provider port implementation to remoting lifecycle events
- **THEN** connected events can become local provider join input while the subscription is retained
- **AND** dropping the subscription stops that adapter from producing further topology input

#### Scenario: remoting subscription does not strongly retain provider

- **WHEN** the caller drops all strong handles to the local provider
- **THEN** the remoting subscription MUST NOT keep the provider alive
- **AND** later remoting lifecycle events are ignored by that adapter

#### Scenario: AWS ECS polling stays provider-owned

- **WHEN** AWS ECS provider starts as member or client
- **THEN** it owns the ECS polling lifecycle and publishes topology updates from discovered running tasks
- **AND** cluster core only observes the resulting topology update events

### Requirement: Downing remains an input boundary for this capability

Provider boundary SHALL allow explicit down or provider-observed departure to produce member departure input. It MUST NOT define failure observation policy, Split Brain Resolver behavior, reachability matrix semantics, rebalance, or remembered entity recovery.

#### Scenario: explicit down is converted to departure input

- **WHEN** a provider accepts explicit down for a current member authority
- **THEN** it produces member departure topology input for that authority
- **AND** Grain runtime invalidation handles stale activation and PID cache removal

#### Scenario: downing decision policy is outside provider boundary

- **WHEN** a failure detector or downing strategy observes a suspected member
- **THEN** this capability does not decide whether the member must be downed
- **AND** any future downing decision model must be specified by a separate capability
