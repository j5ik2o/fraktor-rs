# actor モジュール ギャップ分析

> 分析日: 2026-02-28
> 対象: `modules/actor/src/` vs `references/pekko/actor/src/main/`

## サマリー

| 指標 | 値 |
|---|---:|
| Pekko 公開型数 | 610 |
| fraktor-rs 公開型数 | 405 |
| 同名型カバレッジ | 23/610 (3.8%) |
| ギャップ数（同名差分） | 587 |

> 注: 同名一致ベースのため、別名で実装済みの機能は低く見積もられる。

## 主要ギャップ

| Pekko API | fraktor対応 | 難易度 | 判定 |
|---|---|---|---|
| Router戦略群（RoundRobin/Broadcast/ConsistentHashing/Random/SmallestMailbox） | `Routers::pool` + `PoolRouterBuilderGeneric` | medium | 部分実装 |
| CoordinatedShutdown | 未対応 | hard | 未実装 |
| FSM DSL | 未対応 | medium | 未実装 |
| ReceiveTimeout / PoisonPill / Kill | 未対応 | easy | 未実装 |
| ActorSelection | `ActorSelectionResolver` | - | 別名で実装済み |
| Stash | `StashBufferGeneric` | - | 別名で実装済み |

## 根拠（主要参照）

- Pekko:
  - `references/pekko/actor/src/main/scala/org/apache/pekko/actor/CoordinatedShutdown.scala:41`
  - `references/pekko/actor/src/main/scala/org/apache/pekko/actor/FSM.scala:430`
  - `references/pekko/actor/src/main/scala/org/apache/pekko/routing/RoundRobin.scala:83`
  - `references/pekko/actor/src/main/scala/org/apache/pekko/routing/Broadcast.scala:73`
- fraktor-rs:
  - `modules/actor/src/core/typed/routers.rs:8`
  - `modules/actor/src/core/typed/pool_router_builder.rs:25`
  - `modules/actor/src/core/actor/actor_selection/resolver.rs:15`
  - `modules/actor/src/core/typed/stash_buffer.rs:13`

## 実装優先度提案

1. Phase 1 (easy): `ReceiveTimeout/PoisonPill/Kill` の最小互換
2. Phase 2 (medium): ルーター戦略追加（broadcast/random/hash/smallest-mailbox）
3. Phase 3 (hard): `CoordinatedShutdown` と FSM DSL
