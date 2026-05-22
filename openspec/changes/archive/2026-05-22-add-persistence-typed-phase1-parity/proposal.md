## Why

`docs/gap-analysis/persistence-gap-analysis.md` の Phase 1 では、typed persistence の低コストな Pekko parity gap が 3 件だけ残っている。既存 `PersistenceEffector` は write-side typed API として動くが、typed recovery selection、typed adapter wrapper、typed durable state signal の public contract がないため、Phase 2 の serializer / durable state behavior に進む前の足場が不足している。

## What Changes

- typed persistence crate に Pekko typed `Recovery` / `SnapshotSelectionCriteria` 相当の public API を追加し、既存 `SnapshotCriteria` と混同しない境界を定義する。
- kernel の `ReadEventAdapter` / `WriteEventAdapter` / `EventSeq` を typed API から型付きに扱える `EventAdapter` / typed `EventSeq` 契約を追加し、`SnapshotAdapter` は runtime integration なしの typed snapshot conversion contract として追加する。
- typed durable state API の将来実装で使う `DurableStateSignal` family を追加する。
- `modules/persistence-core-typed` の no_std 境界、1 public type per file、crate root re-export を維持する。
- 実装後に `docs/gap-analysis/persistence-gap-analysis.md` の Phase 1 状態を更新する。

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `persistence-effector-typed-api`: Phase 1 parity として typed recovery selection、typed event / snapshot adapter wrapper、typed durable state signal の public contract を追加する。

## Impact

- `modules/persistence-core-typed/src/`
- `modules/persistence-core-typed/tests/`
- `openspec/specs/persistence-effector-typed-api/spec.md`
- `docs/gap-analysis/persistence-gap-analysis.md`

`modules/persistence-core-kernel` の journal event adapter 契約は再利用対象であり、この change では kernel storage protocol、snapshot-store runtime protocol、serializer contract を変更しない。
