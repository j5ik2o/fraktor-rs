# cluster モジュール ギャップ分析

> 分析日: 2026-02-28
> 対象: `modules/cluster/src/` vs `references/pekko/cluster/src/main/`

## サマリー

| 指標 | 値 |
|---|---:|
| Pekko 公開型数 | 131 |
| fraktor-rs 公開型数 | 177 |
| 同名型カバレッジ | 6/131 (4.6%) |
| ギャップ数（同名差分） | 125 |

> 注: fraktor-rs は cluster に grain/pubsub を内包しており、型数比較は機能統合の影響を受ける。

## 主要ギャップ

| Pekko API | fraktor対応 | 難易度 | 判定 |
|---|---|---|---|
| `joinSeedNodes` / `leave` / `subscribe` / `unsubscribe` | 公開 API は `get/request/down` 中心 | medium | 未実装（公開面） |
| `SplitBrainResolverProvider` | 未対応 | hard | 未実装 |
| `ClusterRouterPool` / `ClusterRouterGroup` | 未対応 | medium | 未実装 |
| `CurrentClusterState` | `CurrentClusterState` | - | 実装済み |
| `DowningProvider` | `DowningProvider` trait（最小） | medium | 部分実装 |

## 根拠（主要参照）

- Pekko:
  - `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/Cluster.scala:354`
  - `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/sbr/SplitBrainResolverProvider.scala:34`
  - `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/routing/ClusterRouterConfig.scala:331`
- fraktor-rs:
  - `modules/cluster/src/core/cluster_api.rs:69`
  - `modules/cluster/src/core/cluster_api.rs:118`
  - `modules/cluster/src/core/membership/current_cluster_state.rs:12`
  - `modules/cluster/src/core/downing_provider.rs:10`

## 実装優先度提案

1. Phase 1 (medium): `join/leave/subscribe/unsubscribe` の公開 API 追加
2. Phase 2 (medium): cluster router（pool/group）を最小導入
3. Phase 3 (hard): split-brain 解決戦略の導入
