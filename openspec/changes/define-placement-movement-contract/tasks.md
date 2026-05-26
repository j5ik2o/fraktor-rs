## 1. Contract audit

- [x] 1.1 `RendezvousHasher` / `PartitionIdentityLookup` / `PlacementCoordinatorCore` の現行 movement behavior を spec と照合する。
- [x] 1.2 join / leave / down / rolling update 時に activation と PID cache がどこで invalidation されるかを確認する。
- [x] 1.3 rebalance / remembered entities / in-flight drain に該当する挙動が本 change に混入しないことを確認する。

## 2. Contract tests

- [x] 2.1 same topology / same `GrainKey` の Rendezvous owner stability を contract test で固定する。
- [x] 2.2 new authority join が既存 active activation を移動せず、cache drop / passivation を出さないことを固定する。
- [x] 2.3 join 後の新規 resolution が expanded topology だけを候補にすることを固定する。
- [x] 2.4 leave / down が matching authority の activation / PID cache だけを invalidation することを固定する。
- [x] 2.5 rolling update が stale authority reuse を防ぎ、rebalance guarantee を持たないことを既存 test と重複しない形で固定する。

## 3. Implementation adjustment

- [x] 3.1 contract tests が露出した不足があれば `PartitionIdentityLookup` / `PlacementCoordinatorCore` の最小修正で解消する。
- [x] 3.2 public API や provider-specific behavior に不要な変更を入れない。
- [x] 3.3 no_std core に `std` 依存や adapter concern が混入していないことを確認する。

## 4. Validation

- [x] 4.1 `define-placement-movement-contract` の OpenSpec validation を実行する。
- [x] 4.2 `cluster-core` の identity / placement targeted tests を実行する。
- [x] 4.3 変更した Markdown / Rust files の formatting checks を実行する。
