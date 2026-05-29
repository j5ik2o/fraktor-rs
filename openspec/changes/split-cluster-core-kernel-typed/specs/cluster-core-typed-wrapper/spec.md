## ADDED Requirements

### Requirement: Typed wrapper crate depends on kernel

`fraktor-cluster-core-typed-rs` SHALL provide typed wrappers over `fraktor-cluster-core-kernel-rs` without owning cluster runtime behavior. The typed crate MUST depend on the kernel crate and MUST NOT introduce a compatibility facade for the removed `fraktor-cluster-core-kernel-rs` crate.

#### Scenario: typed crate wraps kernel

- **WHEN** the workspace dependencies and `modules/cluster-core-typed/Cargo.toml` are inspected
- **THEN** `fraktor-cluster-core-typed-rs` depends on `fraktor-cluster-core-kernel-rs`
- **AND** no `fraktor-cluster-core-kernel-rs` crate is present as a legacy facade

### Requirement: ClusterIdentity typed wrapper converts to kernel

`fraktor-cluster-core-typed-rs` SHALL expose a typed `ClusterIdentity<M>` wrapper that carries a message type marker, validates through the kernel `ClusterIdentity` contract, and supports conversion into the kernel type. The wrapper MUST NOT implement placement, resolution, provider, pub/sub, or Grain call behavior.

#### Scenario: wrapper preserves kernel validation

- **WHEN** a typed `ClusterIdentity<M>` is constructed with empty kind or identity input
- **THEN** construction fails through the same validation contract as kernel `ClusterIdentity`
- **AND** valid typed identities can be converted into kernel `ClusterIdentity`

#### Scenario: typed wrapper does not own behavior

- **WHEN** `modules/cluster-core-typed/src` is inspected
- **THEN** it contains wrapper code for `ClusterIdentity`
- **AND** it does not contain implementations for cluster placement, resolution, provider, pub/sub, or Grain call behavior
