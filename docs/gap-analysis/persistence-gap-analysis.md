# persistence モジュール ギャップ分析

> 分析日: 2026-02-28
> 対象: `modules/persistence/src/` vs `references/pekko/persistence/src/main/`

## サマリー

| 指標 | 値 |
|---|---:|
| Pekko 公開型数 | 154 |
| fraktor-rs 公開型数 | 56 |
| 同名型カバレッジ | 25/154 (16.2%) |
| ギャップ数（同名差分） | 129 |

> 注: 同名一致では低く見えるが、`Journal` / `SnapshotStore` / `PersistentActor` 中核は実装済み。

## 主要ギャップ

| Pekko API | fraktor対応 | 難易度 | 判定 |
|---|---|---|---|
| PersistentFSM | 未対応 | medium | 未実装 |
| PersistencePluginProxy | 未対応 | medium | 未実装 |
| AsyncWriteJournal | `Journal` trait (GAT) | easy | 別名で実装済み |
| SnapshotOffer | `SnapshotResponse::LoadSnapshotResult` で近似 | easy | 部分実装 |
| persistAsync | `persist_unfenced` / `persist_all_async` | trivial | 別名で実装済み |

## 根拠（主要参照）

- Pekko:
  - `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/fsm/PersistentFSM.scala:78`
  - `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/journal/PersistencePluginProxy.scala:38`
  - `references/pekko/persistence/src/main/scala/org/apache/pekko/persistence/PersistentActor.scala:242`
- fraktor-rs:
  - `modules/persistence/src/core/persistent_actor.rs:26`
  - `modules/persistence/src/core/persistent_actor.rs:54`
  - `modules/persistence/src/core/persistent_actor.rs:93`
  - `modules/persistence/src/core/journal.rs:8`
  - `modules/persistence/src/core/snapshot_response.rs:27`

## 実装優先度提案

1. Phase 1 (trivial/easy): `persist_async` 互換エイリアス名を追加
2. Phase 2 (medium): `PersistentFSM` 相当レイヤ追加
3. Phase 3 (medium): `PersistencePluginProxy` 相当のプラグイン透過層追加
