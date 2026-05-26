## 1. Test Placement

- [x] 1.1 Review existing `modules/cluster-core/src/identity/grain_runtime_operational_contract_test.rs` helpers for pending activation command completion.
- [x] 1.2 Add a focused contract test in the existing identity operational contract test module for first public `resolve` returning `LookupError::Pending`.
- [x] 1.3 Add a repeated public `resolve` assertion while the placement command sequence is still outstanding.
- [x] 1.4 Complete the emitted placement command sequence and assert a later public `resolve` returns the stored PID and selected authority.

## 2. Minimal Implementation Fixes

- [x] 2.1 Run the new focused test first and inspect whether current behavior already satisfies the contract.
- [x] 2.2 If the test fails, adjust only `PartitionIdentityLookup` / placement coordinator behavior needed to preserve pending resolution until command completion.
- [x] 2.3 Confirm the fix does not broaden topology invalidation, passivation, rolling update, rebalance, remembered entities, or SBR scope.

## 3. Validation

- [x] 3.1 Run the focused `cluster-core` identity operational contract test.
- [x] 3.2 Run the relevant `cluster-core` test package target if available.
- [x] 3.3 Run `MISE_TRUSTED_CONFIG_PATHS=$PWD/mise.toml mise exec -- openspec validate test-grain-pending-activation-contract --strict`.
- [x] 3.4 Run `git diff --check`.
