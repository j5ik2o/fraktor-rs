## 1. Contract Test Baseline

- [ ] 1.1 Review existing `PartitionIdentityLookup`, `PlacementCoordinatorCore`, `VirtualActorRegistry`, and `PidCache` tests against the new spec scenarios.
- [ ] 1.2 Add a focused contract test module for Grain runtime operational behavior instead of scattering new scenarios across unrelated unit tests.
- [ ] 1.3 Ensure contract tests use public or crate-level cluster APIs that match intended caller boundaries.

## 2. Identity Resolution Contract

- [ ] 2.1 Add tests for no-authority resolution returning `LookupError::NoAuthority` without caching a PID.
- [ ] 2.2 Add tests proving deterministic authority selection for identical topology and `GrainKey`.
- [ ] 2.3 Add tests proving repeated lookup returns the same active cached PID before TTL or passivation invalidates it.
- [ ] 2.4 Add tests for distributed activation pending flow and completion after command results.

## 3. Topology And Member Departure Contract

- [ ] 3.1 Add tests proving topology replacement invalidates activation and PID cache entries for absent authorities.
- [ ] 3.2 Add tests proving `on_member_left` invalidates entries for the departed authority.
- [ ] 3.3 Add tests proving unknown member departure does not invalidate unrelated active entries.
- [ ] 3.4 Add assertions for emitted placement or PID cache events where the contract requires observability.

## 4. Passivation Contract

- [ ] 4.1 Add tests proving idle activation passivation removes activation state and PID cache entries.
- [ ] 4.2 Add tests proving recent activation remains reusable when idle TTL has not elapsed.
- [ ] 4.3 Add tests proving passivated keys resolve through a new placement decision rather than stale cache hit.

## 5. Rolling Update Boundary

- [ ] 5.1 Add a scenario test for old authority removal and replacement authority re-resolution.
- [ ] 5.2 Document in test names or comments that rolling update guarantees stale authority invalidation, not rebalance or remembered entity recovery.
- [ ] 5.3 Avoid adding rebalance, remembered entity, SBR, or reachability matrix implementation in this change.

## 6. Verification

- [ ] 6.1 Run targeted cluster-core tests for identity, placement, grain registry, and the new operational contract module.
- [ ] 6.2 Run `MISE_TRUSTED_CONFIG_PATHS=$PWD/mise.toml mise exec -- openspec validate stabilize-grain-runtime-operational-contract --strict`.
- [ ] 6.3 Run formatting checks for touched Rust and Markdown files.
