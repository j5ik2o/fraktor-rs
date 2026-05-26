## Why

The cluster roadmap now positions `cluster-*` as a Proto.Actor-Go style Virtual Actor / Grain runtime, but some public docs still read as broad Apache Pekko Cluster parity material. Aligning the docs prevents future work from treating Cluster Singleton, ShardCoordinator, Distributed Data, or raw Pekko API parity as the immediate implementation target.

## What Changes

- Reframe cluster documentation so the primary subject is Grain identity lookup, placement resolution, activation/passivation, topology input, provider boundaries, and failure/downing contracts.
- Treat Pekko Cluster / Cluster Sharding as a reference for operational failure cases and design vocabulary, not as a parity target.
- Add explicit roadmap links from README / gap-analysis documentation to `docs/plan/2026-05-25_cluster-grain-runtime-roadmap.md`.
- Mark Cluster Singleton, ShardCoordinator, Cluster Client, Receptionist, Distributed Data / CRDT, and sharding delivery controllers as deferred unless a future OpenSpec change explicitly adopts them.
- Preserve existing gap-analysis evidence where useful, but relabel it as comparison context instead of direct priority.

## Capabilities

### New Capabilities

- `cluster-grain-runtime-docs`: Documentation contract for presenting cluster as a Grain runtime and Pekko as a reference implementation, including required links, deferred scope language, and consistency across README / gap-analysis / roadmap docs.

### Modified Capabilities

- None.

## Impact

- Affected docs: `README.md`, `docs/gap-analysis/cluster-gap-analysis.md`, and related cluster roadmap links under `docs/plan/`.
- No Rust API, runtime behavior, dependency, or provider implementation changes.
- No migration or compatibility impact.
