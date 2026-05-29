## 1. OpenSpec and Package Setup

- [x] 1.1 Validate the `split-cluster-core-kernel-typed` OpenSpec change before implementation.
- [x] 1.2 Rename `modules/cluster-core` to `modules/cluster-core-kernel`.
- [x] 1.3 Update `cluster-core-kernel` package metadata from `fraktor-cluster-core-rs` to `fraktor-cluster-core-kernel-rs`.
- [x] 1.4 Add `modules/cluster-core-typed` package metadata and no_std crate root.

## 2. Kernel Dependency Migration

- [x] 2.1 Update workspace members and workspace dependency aliases for kernel and typed crates.
- [x] 2.2 Update `cluster-adaptor-std`, showcases, tests, and docs to use `fraktor-cluster-core-kernel-rs`.
- [x] 2.3 Verify no compatibility facade or dependency alias remains for `fraktor-cluster-core-rs`.

## 3. Typed Wrapper

- [x] 3.1 Add typed `ClusterIdentity<M>` wrapper over kernel `ClusterIdentity`.
- [x] 3.2 Add conversion APIs between `ClusterIdentity<M>` and kernel `ClusterIdentity`.
- [x] 3.3 Add typed wrapper tests covering validation and conversion.

## 4. Verification

- [x] 4.1 Run `MISE_TRUSTED_CONFIG_PATHS=$PWD/mise.toml mise exec -- openspec validate split-cluster-core-kernel-typed --strict`.
- [x] 4.2 Run `cargo test -p fraktor-cluster-core-kernel-rs`.
- [x] 4.3 Run `cargo test -p fraktor-cluster-core-typed-rs`.
- [x] 4.4 Run `cargo test -p fraktor-cluster-adaptor-std-rs`.
- [x] 4.5 Run `cargo check --workspace`.
- [x] 4.6 Run `cargo fmt --check --all` and `git diff --check`.
