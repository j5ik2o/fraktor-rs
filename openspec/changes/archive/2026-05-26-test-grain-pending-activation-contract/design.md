## Context

`cluster-grain-runtime-operational-contract` already requires distributed activation to expose pending resolution until lock/load/ensure/store/release commands complete. The existing operational test module drives part of that flow through a test-only coordinator outcome helper, but the public `IdentityLookup::resolve` behavior should also be pinned directly.

This change is intentionally test-first. It should use the existing `PartitionIdentityLookup` and placement command result APIs, preserving the `no_std` core boundary and avoiding new dependencies.

## Goals / Non-Goals

**Goals:**

- Add contract tests that call public `resolve` while distributed activation is incomplete.
- Prove unresolved activation is reported as `LookupError::Pending`.
- Prove repeated `resolve` calls do not fabricate a PID or skip the outstanding activation flow.
- Prove a later `resolve` returns the stored PID after the emitted placement command sequence is completed.

**Non-Goals:**

- Do not add new public APIs solely for tests.
- Do not implement topology invalidation, passivation, rolling update, rebalance, remembered entities, or SBR behavior.
- Do not move test support into `std` or introduce async runtime dependencies.
- Do not broaden Pekko Cluster / Cluster Sharding API parity.

## Decisions

1. Keep the tests in the existing cluster-core identity test area.

   `modules/cluster-core/src/identity/grain_runtime_operational_contract_test.rs` already contains the Grain runtime operational contract tests and helper functions for placement command completion. Extending that file keeps the new scenarios near the accepted operational contract.

2. Use public `IdentityLookup::resolve` for pending assertions.

   The first pending assertion should go through `resolve`, not only `resolve_outcome`, because callers observe `LookupError::Pending` through the public identity lookup trait. The existing test-only outcome helper may still be used after that to capture the command request id needed to complete the activation flow.

3. Treat implementation changes as defect fixes only.

   If the tests fail, the implementation should be adjusted only where the public pending-resolution contract is violated. The change must not redesign placement coordination or introduce new behavior outside the pending activation path.

## Risks / Trade-offs

- Existing helper functions may hide part of the command sequence -> Keep assertions explicit about the public `resolve` calls before and after command completion.
- Test-only access to coordinator internals can become too broad -> Reuse the existing helper instead of adding new public or test-only seams.
- Repeated `resolve` while a command is outstanding may expose current behavior that starts a duplicate activation -> Fix only if that violates the pending contract and keep the scope limited to the outstanding activation state.
