## Context

`fraktor-cluster-core-kernel-rs` already contains the cluster runtime body after the module reorganization change. Other package families expose the implementation body through a kernel crate and use typed crates as wrappers over that body. Cluster should follow the same direction: kernel owns behavior, typed wraps kernel.

This change is breaking because the old crate name and directory are replaced. The repository is pre-release, so no compatibility facade, deprecated alias, or forwarding crate is kept.

## Goals / Non-Goals

**Goals:**

- Move the existing `cluster-core` crate body to `cluster-core-kernel` with behavior unchanged.
- Add `cluster-core-typed` as a wrapper crate depending on kernel.
- Keep existing dependents on kernel unless they explicitly need typed wrappers.
- Add the first typed wrapper around kernel `ClusterIdentity` as `ClusterIdentity<M>`.
- Update docs, OpenSpec, tests, workspace members, and dependency aliases to the new crate names.

**Non-Goals:**

- Do not move cluster behavior from kernel into typed.
- Do not create a legacy `fraktor-cluster-core-kernel-rs` facade.
- Do not introduce new dependencies from cluster typed to actor typed or other typed crates.
- Do not add a fully typed Grain call API in this change.

## Decisions

### Decision 1: Kernel owns the existing cluster body

All current cluster-core modules remain implementation modules under `cluster-core-kernel`. This includes `ClusterApi`, `ClusterExtension`, `GrainRef`, membership, placement, provider, pub/sub, downing, and topology. The split is package-level, not a behavioral refactor.

Alternative considered: move `ActorSystem` entrypoints or Grain caller API into typed. This was rejected because typed is a wrapper over kernel, not the location for the core logic.

### Decision 2: Typed starts with a minimal wrapper

`cluster-core-typed` starts with `ClusterIdentity<M>`, a typed wrapper for kernel `ClusterIdentity` that carries the message type marker. It owns only wrapper semantics and conversions to/from kernel types. It does not re-export all kernel modules and does not duplicate cluster behavior.

Alternative considered: create an empty typed crate. This was rejected because a crate with no wrapper contract has little verification value.

### Decision 3: Existing dependents migrate to kernel

`cluster-adaptor-std`, showcases, and existing tests use current behavior, so they migrate to `fraktor-cluster-core-kernel-rs`. Typed usage is limited to new typed-wrapper tests.

Alternative considered: switch some existing users to typed during the split. This was rejected to avoid mixing behavior migration with wrapper adoption.

## Risks / Trade-offs

- [Risk] Large path move hides behavior changes. -> Keep the kernel move mechanical and verify targeted cluster tests.
- [Risk] Typed wrapper becomes a facade. -> Export only the wrapper and necessary conversion surface.
- [Risk] Documentation still points to old crate names. -> Search `fraktor-cluster-core-kernel-rs`, `fraktor_cluster_core_kernel_rs`, and `modules/cluster-core-kernel` after edits.
- [Risk] OpenSpec archive/spec drift from the just-archived module organization. -> Validate the new change and the modified capability strictly.

## Migration Plan

1. Archive the completed module reorganization change.
2. Create `split-cluster-core-kernel-typed` OpenSpec artifacts.
3. Rename `modules/cluster-core-kernel` to `modules/cluster-core-kernel` and update package metadata.
4. Add `modules/cluster-core-typed` with the minimal `ClusterIdentity<M>` wrapper and tests.
5. Update workspace members, dependencies, imports, docs, and specs.
6. Run targeted tests, workspace check, formatting, and diff checks.
