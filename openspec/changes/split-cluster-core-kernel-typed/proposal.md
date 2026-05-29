## Why

`fraktor-cluster-core-kernel-rs` currently contains the cluster runtime body and has not adopted the kernel / typed split used by other core packages. Splitting it clarifies that cluster behavior lives in kernel while typed is only a wrapper layer over kernel contracts.

## What Changes

- **BREAKING**: Replace `fraktor-cluster-core-kernel-rs` / `modules/cluster-core-kernel` with `fraktor-cluster-core-kernel-rs` / `modules/cluster-core-kernel`.
- **BREAKING**: Update existing workspace dependents, tests, examples, and docs to import `fraktor-cluster-core-kernel-rs`; do not keep a legacy facade or deprecated aliases.
- Add `fraktor-cluster-core-typed-rs` / `modules/cluster-core-typed` as a thin wrapper crate that depends on kernel.
- Keep existing cluster runtime logic, including `ClusterCore`, `ClusterApi`, `ClusterExtension`, `GrainRef`, membership, placement, provider, pub/sub, downing, and topology behavior in kernel.
- Introduce the first typed wrapper around kernel `ClusterIdentity` as `ClusterIdentity<M>`; the wrapper carries the message type marker, converts to and from the kernel type, and does not implement cluster behavior.

## Capabilities

### New Capabilities
- `cluster-core-typed-wrapper`: Defines the typed wrapper crate contract over cluster-core-kernel.

### Modified Capabilities
- `cluster-core-module-organization`: Updates the cluster core source and package organization contract from a single `cluster-core` crate to kernel / typed crates.

## Impact

- Workspace membership and dependency aliases in `Cargo.toml`.
- `modules/cluster-core-kernel` is moved to `modules/cluster-core-kernel`; `modules/cluster-core-typed` is added.
- `cluster-adaptor-std`, `showcases/std`, cluster tests, README files, docs, and OpenSpec references are updated from `fraktor-cluster-core-kernel-rs` / `modules/cluster-core-kernel` to kernel paths where they use existing behavior.
