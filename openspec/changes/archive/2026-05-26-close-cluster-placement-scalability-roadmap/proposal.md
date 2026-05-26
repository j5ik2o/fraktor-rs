## Why

`docs/plan/2026-05-25_cluster-grain-runtime-roadmap.md` still shows part of Task slice 5 as open, even though `define-placement-movement-contract` and `test-grain-pending-activation-contract` have already fixed the relevant Placement scalability contracts.
This change closes that roadmap drift so the remaining cluster work is not misread as an implementation gap.

## What Changes

- Mark the remaining Placement scalability roadmap item as completed where current specs and archived changes already cover it.
- Link the roadmap status to the completed placement movement and pending activation contract work.
- Keep future rebalance, remembered entities, recovery, and in-flight drain out of scope.
- Do not change Rust source, public APIs, runtime behavior, dependencies, or OpenSpec requirements.

## Capabilities

### New Capabilities

- なし

### Modified Capabilities

- `cluster-grain-runtime-operational-contract`: Clarify that the existing Rendezvous / movement contract is the bounded Placement scalability contract for this roadmap slice.

## Impact

- `docs/plan/2026-05-25_cluster-grain-runtime-roadmap.md`
- OpenSpec change artifacts only
