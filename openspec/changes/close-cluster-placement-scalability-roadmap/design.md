## Context

`docs/plan/2026-05-25_cluster-grain-runtime-roadmap.md` still shows "Rendezvous hashing のまま伸ばす範囲を明確にする" as open.
Current `openspec/specs/cluster-grain-runtime-operational-contract/spec.md` already contains the corresponding contract:

- deterministic Rendezvous owner selection for the same key and topology
- no immediate movement for existing active activations on join
- expanded topology use for new resolutions after join
- no minimum movement guarantee across topology changes
- rolling update bounded to stale placement prevention

The archived `define-placement-movement-contract` change and the completed `test-grain-pending-activation-contract` change cover the remaining placement-related contract gaps.

## Goals / Non-Goals

**Goals:**

- Update the roadmap so Task slice 5 reflects current completed placement contracts.
- Point readers at the completed OpenSpec work that closed the Rendezvous / movement scope.
- Keep the roadmap useful for deciding future rebalance or remembered entity work.

**Non-Goals:**

- Add or modify OpenSpec requirements.
- Change placement algorithm behavior.
- Implement rebalance, remembered entities, persistence recovery, or in-flight drain.
- Modify Rust source or public APIs.

## Decisions

1. Treat this as a docs-only roadmap alignment change.

   The behavioral contract already exists in `cluster-grain-runtime-operational-contract`, so adding another capability would duplicate the source of truth.
   Alternative: create a new placement scalability capability. That would make the same constraints appear in two places and increase archive drift risk.

2. Mark the roadmap item complete instead of creating new implementation tasks.

   The remaining visible item is stale roadmap state, not missing runtime behavior.
   Alternative: leave it open as a reminder for future rebalance work. That is misleading because rebalance / remembered entities are already deferred scope, not part of the current Placement scalability slice.

3. Keep future scalability work separate.

   Real rebalance, remembered entities, recovery, and draining require their own contracts and should not be bundled into this cleanup.

## Risks / Trade-offs

- [Risk] Readers may think all future placement scalability work is done. -> Mitigation: keep deferred scope explicit and reference that rebalance / remembered entities remain future changes.
- [Risk] Docs-only change may look too small for OpenSpec. -> Mitigation: this change exists because the roadmap is being used as work-tracking input and needs an auditable closure.
