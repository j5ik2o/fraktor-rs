## Why

`cluster-grain-runtime-operational-contract` は topology invalidation と rolling update の最小境界を持っているが、Rendezvous hashing のまま伸ばす範囲と、join / leave 時の placement movement expectation はまだ明文化が薄い。
rebalance や remembered entities を実装する前に、現在の Grain runtime が保証する movement と保証しない movement を contract として固定する。

## What Changes

- Rendezvous hashing による placement decision の安定性と、同一 topology / same key での deterministic selection を明確化する。
- node join 時は既存 active activation を即時 rebalance しないことを固定する。
- node leave / down 時は departed authority の activation / PID cache を invalidation し、次回 resolution が active topology だけを使うことを固定する。
- rolling update 時に replacement topology へ再解決されることと、minimum movement / remembered entity recovery / in-flight drain を保証しないことを明確化する。
- 将来の rebalance / remembered entities / draining は別 capability に残す。

## Capabilities

### New Capabilities

- なし

### Modified Capabilities

- `cluster-grain-runtime-operational-contract`: placement movement と Rendezvous hashing の bounded contract を追加する。

## Impact

- `openspec/specs/cluster-grain-runtime-operational-contract/spec.md`
- `modules/cluster-core/src/identity/grain_runtime_operational_contract_test.rs`
- `modules/cluster-core/src/identity/rendezvous_hasher.rs`
- `modules/cluster-core/src/identity/partition_identity_lookup.rs`
- `modules/cluster-core/src/placement/placement_coordinator.rs`
