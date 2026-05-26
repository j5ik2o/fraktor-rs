# cluster-grain-runtime-docs Specification

## Purpose
Define the documentation contract that presents `cluster-*` as Grain runtime infrastructure and treats Apache Pekko Cluster material as operational comparison context rather than the current parity roadmap.

## Requirements
### Requirement: Cluster docs present Grain runtime as the primary model

Cluster documentation SHALL describe `cluster-*` as Virtual Actor / Grain runtime infrastructure centered on identity lookup, placement resolution, activation/passivation, topology updates, provider boundaries, failure observation, and downing decisions. It MUST NOT present Apache Pekko Cluster or Cluster Sharding public API parity as the current cluster roadmap.

#### Scenario: root README summarizes cluster direction

- **WHEN** a reader uses `README.md` to understand cluster support
- **THEN** the cluster entry describes the Grain runtime direction
- **AND** it links to the cluster Grain runtime roadmap for current priority

#### Scenario: cluster gap analysis is comparison context

- **WHEN** a reader opens `docs/gap-analysis/cluster-gap-analysis.md`
- **THEN** the document states that Pekko is used as a reference for operational concerns
- **AND** it does not treat raw Pekko API gaps as direct implementation priority

### Requirement: Deferred Pekko cluster concepts are explicit

Cluster documentation SHALL explicitly defer Cluster Singleton, ShardCoordinator parity, Cluster Client, Receptionist, Distributed Data / CRDT, sharding delivery controllers, and broad Pekko public API compatibility unless a future OpenSpec change adopts one of those scopes.

#### Scenario: deferred scope is visible before gap tables

- **WHEN** a reader reaches cluster gap-analysis detail tables
- **THEN** the document identifies which Pekko concepts are deferred
- **AND** the reader can distinguish deferred comparison entries from current Grain runtime tasks

#### Scenario: future implementation work requires a new change

- **WHEN** documentation mentions a deferred Pekko cluster concept
- **THEN** it states or links that implementation requires a dedicated future OpenSpec change

### Requirement: Cluster documentation links remain consistent

Cluster documentation SHALL keep the roadmap, gap analysis, and top-level README connected so readers can move from overview to rationale to detailed comparison without conflicting priority statements.

#### Scenario: overview points to rationale

- **WHEN** top-level documentation mentions cluster roadmap or gap analysis
- **THEN** it links to `docs/plan/2026-05-25_cluster-grain-runtime-roadmap.md` or the cluster gap analysis as appropriate

#### Scenario: detailed comparison points back to roadmap

- **WHEN** the cluster gap analysis discusses implementation priority
- **THEN** it points back to the cluster Grain runtime roadmap as the authoritative priority document
