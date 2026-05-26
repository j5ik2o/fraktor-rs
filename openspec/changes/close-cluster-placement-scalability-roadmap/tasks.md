## 1. Roadmap Alignment

- [ ] 1.1 Confirm `define-placement-movement-contract` is archived and reflected in `cluster-grain-runtime-operational-contract`.
- [ ] 1.2 Confirm `test-grain-pending-activation-contract` is complete and no longer leaves an open pending activation roadmap gap.
- [ ] 1.3 Update `docs/plan/2026-05-25_cluster-grain-runtime-roadmap.md` so Task slice 5 marks the Rendezvous hashing scope as complete.
- [ ] 1.4 Keep future rebalance, remembered entities, persistence recovery, and in-flight drain listed only as deferred or future scope.

## 2. Specification Alignment

- [ ] 2.1 Apply the spec clarification that the existing Rendezvous / movement requirement is the bounded Placement scalability contract.
- [ ] 2.2 Ensure the clarification does not introduce a new placement algorithm, rebalance guarantee, or public API requirement.

## 3. Validation

- [ ] 3.1 Run `MISE_TRUSTED_CONFIG_PATHS=$PWD/mise.toml mise exec -- openspec validate close-cluster-placement-scalability-roadmap --strict`.
- [ ] 3.2 Run `git diff --check`.
- [ ] 3.3 Review the final diff and confirm only OpenSpec artifacts and roadmap documentation changed.
