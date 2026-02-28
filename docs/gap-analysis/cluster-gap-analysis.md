# cluster モジュール ギャップ分析

> 分析日: 2026-02-27（前回: 2026-02-24）
> 対象: `modules/cluster/src/` vs `references/pekko/cluster-typed/src/` + `references/pekko/cluster/src/`

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（公開API + 主要内部型） | 約45型 |
| fraktor-rs 公開型数 | 約160型 |
| カバレッジ（機能カテゴリ単位） | 8/12 (67%)（前回 7/12 → 改善） |
| 主要ギャップ数 | 14（前回15 → 1件削減） |

> 注: fraktor-rsのclusterモジュールはPekkoよりもprotoactor-goの設計に近い。Pekkoでは別モジュール（cluster-sharding, distributed-pub-sub）に分離されている機能（Virtual Actors, Pub/Sub）がfraktor-rsではclusterモジュールに統合されている。型数の差はこの設計差異による。

### 前回分析からの変更

以下の機能が新たに実装済みとなった：
- `DowningProvider` trait → 実装済み（`downing_provider.rs`）。`NoopDowningProvider` が既定実装

## 設計方針の差異

| 観点 | Pekko | fraktor-rs | 備考 |
|------|-------|-----------|------|
| Virtual Actors | cluster-sharding（別モジュール） | Grain（cluster内蔵） | protoactor-go準拠 |
| Pub/Sub | distributed-pub-sub（別モジュール） | PubSub（cluster内蔵） | protoactor-go準拠 |
| メンバーシップ | Gossipプロトコル + VectorClock | Gossip + MembershipVersion（単調増加） | 簡略化された一貫性モデル |
| 障害検出 | FailureDetector統合 + Reachability | PhiFailureDetector（remote経由） | 基本機能は実装済み |
| ダウニング | プラガブルDowningProvider + SBR | DowningProvider trait + NoopDowningProvider | 基盤のみ実装。`DowningProvider::down` が下流の明示ダウンを制御し、最終的な適用は `ClusterProvider::down` へ委譲 |
| イベントモデル | 豊富なsealed trait階層 | ClusterEvent enum | 簡略化 |

## カテゴリ別ギャップ

### 1. クラスタ管理（Cluster Extension）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Cluster` (Extension) | `Cluster.scala` | `ClusterExtensionGeneric` | - | 実装済み |
| `Cluster.subscribe` | `Cluster.scala` | EventStream経由 | - | パターンは異なるが機能的に同等 |
| `Cluster.join` | `Cluster.scala` | `start_member` / `start_client` | - | API名は異なるが概念は対応 |
| `Cluster.leave` | `Cluster.scala` | `shutdown(graceful=true)` | - | 実装済み |
| `Cluster.down` | `Cluster.scala` | `DowningProvider::down()` | - | **実装済み**（DowningProvider trait経由） |
| `Cluster.prepareForFullClusterShutdown` | `Cluster.scala` | 未対応 | medium | 全ノード協調シャットダウン |
| `Cluster.registerOnMemberUp` | `Cluster.scala` | 未対応 | easy | コールバック登録。EventStreamで代替可 |
| `Cluster.registerOnMemberRemoved` | `Cluster.scala` | 未対応 | easy | コールバック登録。EventStreamで代替可 |

### 2. メンバー表現（Member）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Member` class | `Member.scala` | `NodeRecord` | - | 別名で実装済み。フィールド構成は異なる |
| `MemberStatus` (sealed) | `Member.scala` | `NodeStatus` enum | 部分 | 下記参照 |
| `MemberStatus.Joining` | `Member.scala` | `NodeStatus::Joining` | - | 実装済み |
| `MemberStatus.WeaklyUp` | `Member.scala` | 未対応 | medium | 部分的到達可能性下でのUp。Reachability前提 |
| `MemberStatus.Up` | `Member.scala` | `NodeStatus::Up` | - | 実装済み |
| `MemberStatus.Leaving` | `Member.scala` | `NodeStatus::Leaving` | - | 実装済み |
| `MemberStatus.Exiting` | `Member.scala` | 未対応 | easy | Leaving後の遷移状態 |
| `MemberStatus.Down` | `Member.scala` | `NodeStatus::Dead` | - | 別名で実装済み |
| `MemberStatus.Removed` | `Member.scala` | `NodeStatus::Removed` | - | 実装済み |
| `MemberStatus.PreparingForShutdown` | `Member.scala` | 未対応 | medium | 全クラスタシャットダウン前提 |
| `MemberStatus.ReadyForShutdown` | `Member.scala` | 未対応 | medium | 全クラスタシャットダウン前提 |
| `UniqueAddress` | `Member.scala` | `authority: String` | - | 簡略化。String型で表現 |
| `Member.roles` | `Member.scala` | 未対応 | easy | ロールベースのメンバー分類 |
| `Member.appVersion` | `Member.scala` | 未対応 | trivial | バージョン情報の付与 |
| `Member.isOlderThan` | `Member.scala` | 未対応 | easy | メンバー年齢比較（リーダー選出に使用） |

### 3. クラスタイベント（ClusterEvent）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `ClusterDomainEvent` trait | `ClusterEvent.scala` | `ClusterEvent` enum | - | 実装済み（enum化） |
| `CurrentClusterState` | `ClusterEvent.scala` | `MembershipSnapshot` | - | 別名で部分実装。`leader`, `roleLeaderMap`, `unreachable` が不足 |
| `MemberJoined` | `ClusterEvent.scala` | `MemberStatusChanged` | - | 汎用variant |
| `MemberUp` | `ClusterEvent.scala` | `MemberStatusChanged` | - | 汎用variant |
| `MemberLeft` | `ClusterEvent.scala` | `MemberStatusChanged` | - | 汎用variant |
| `MemberExited` | `ClusterEvent.scala` | 未対応 | easy | Exiting状態が未対応のため |
| `MemberRemoved` | `ClusterEvent.scala` | `MemberStatusChanged` | - | 汎用variant |
| `MemberDowned` | `ClusterEvent.scala` | `MemberStatusChanged` | - | 汎用variant |
| `MemberWeaklyUp` | `ClusterEvent.scala` | 未対応 | medium | WeaklyUp状態が未対応のため |
| `MemberPreparingForShutdown` | `ClusterEvent.scala` | 未対応 | medium | シャットダウン準備状態が未対応 |
| `MemberReadyForShutdown` | `ClusterEvent.scala` | 未対応 | medium | シャットダウン準備状態が未対応 |
| `LeaderChanged` | `ClusterEvent.scala` | 未対応 | medium | リーダー選出機能が未実装 |
| `RoleLeaderChanged` | `ClusterEvent.scala` | 未対応 | medium | ロール別リーダーが未実装 |
| `UnreachableMember` | `ClusterEvent.scala` | `MemberQuarantined` | 部分 | quarantineとunreachableは異なる概念 |
| `ReachableMember` | `ClusterEvent.scala` | 未対応 | medium | Reachability復帰イベント |
| `DataCenterReachabilityChanged` | `ClusterEvent.scala` | 未対応 | n/a | マルチDC未対応 |
| `SeenChanged` | `ClusterEvent.scala` | 未対応 | medium | Gossipコンバージェンス進捗 |

### 4. Gossipプロトコル

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Gossip` (immutable state) | `Gossip.scala` | `GossipDisseminationCoordinator` | - | 異なる設計だが同等機能 |
| `GossipOverview` | `Gossip.scala` | 未対応（概念なし） | n/a | fraktor-rsはdelta-gossip方式 |
| `VectorClock` | `VectorClock.scala` | `MembershipVersion` (monotonic u64) | 部分 | 因果順序ではなく単調バージョン。分散環境での順序保証が弱い |
| `Gossip.merge` | `Gossip.scala` | `apply_incoming` | - | 機能的に同等 |
| `Gossip.seen` | `Gossip.scala` | 未対応 | medium | Seen tracking（コンバージェンス用） |
| `GossipEnvelope` | `Gossip.scala` | `GossipOutbound` | - | 別名で実装済み |

### 5. 到達可能性（Reachability）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Reachability` class | `Reachability.scala` | 未対応 | hard | Observer/Subject モデルの到達可能性追跡 |
| `Reachability.Record` | `Reachability.scala` | 未対応 | hard | 個別の観測記録 |
| `ReachabilityStatus` (sealed) | `Reachability.scala` | `NodeStatus::Suspect` | 部分 | Suspect≈Unreachable。Terminated状態なし |
| `Reachability.allUnreachable` | `Reachability.scala` | 未対応 | hard | 複数observerの集約判定 |

### 6. 障害検出・ダウニング（Failure Detection & Downing）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `DowningProvider` (abstract) | `DowningProvider.scala` | `DowningProvider` trait | - | **実装済み**。`NoopDowningProvider` が既定 |
| `SplitBrainResolver` | `sbr/SplitBrainResolver.scala` | 未対応 | hard | ネットワーク分断時の自動ダウニング判定。proto.actor-goベースの明示ダウン経路では対応外 |
| `DowningStrategy` (abstract) | `sbr/DowningStrategy.scala` | 未対応 | hard | ダウニング判定ロジックの抽象化 |
| `DowningStrategy.Decision` (sealed) | `sbr/DowningStrategy.scala` | 未対応 | hard | DownReachable, DownUnreachable, DownAll 等 |
| Phi Accrual FD統合 | `Cluster.scala` | PhiFailureDetector（remote経由） | - | 実装済み。MembershipCoordinatorが使用 |

### 7. ハートビート

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `ClusterHeartbeatSender` | `ClusterHeartbeat.scala` | `TokioGossiper`（部分的） | 部分 | Gossip周期にハートビートを統合 |
| `ClusterHeartbeatReceiver` | `ClusterHeartbeat.scala` | `handle_heartbeat` | - | MembershipCoordinator内で処理 |
| `Heartbeat` message | `ClusterHeartbeat.scala` | Gossipメッセージに統合 | - | 別個のメッセージ型なし |
| `HeartbeatRsp` message | `ClusterHeartbeat.scala` | Gossipメッセージに統合 | - | 別個のメッセージ型なし |
| `CrossDcClusterHeartbeatSender` | `CrossDcClusterHeartbeat.scala` | 未対応 | n/a | マルチDC未対応 |
| `CrossDcClusterHeartbeatReceiver` | `CrossDcClusterHeartbeat.scala` | 未対応 | n/a | マルチDC未対応 |

### 8. リーダー選出・コンバージェンス

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `MembershipState.isLeader` | `MembershipState.scala` | 未対応 | medium | リーダー判定ロジック |
| `MembershipState.leaderOf` | `MembershipState.scala` | 未対応 | medium | ロール別リーダー |
| `MembershipState.convergence` | `MembershipState.scala` | 未対応 | hard | Gossipコンバージェンス判定 |
| `MembershipState.seen` | `MembershipState.scala` | 未対応 | medium | Seen集合の管理 |

### 9. 設定・構成

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `ClusterSettings` | `ClusterSettings.scala` | `ClusterExtensionConfig` | - | 実装済み（Builder パターン） |
| `ClusterSettings.Roles` | `ClusterSettings.scala` | 未対応 | easy | ロール設定 |
| `ClusterSettings.SelfDataCenter` | `ClusterSettings.scala` | 未対応 | n/a | マルチDC未対応 |
| `ClusterSettings.FailureDetectorConfig` | `ClusterSettings.scala` | `MembershipCoordinatorConfig` | - | 別名で実装済み |

### 10. ルーティング

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `ClusterRouterGroupSettings` | `routing/ClusterRouterConfig.scala` | 未対応 | hard | クラスタ対応ルーティング |
| `ClusterRouterPoolSettings` | `routing/ClusterRouterConfig.scala` | 未対応 | hard | クラスタ対応プールルーティング |
| `ClusterScope` | `ClusterActorRefProvider.scala` | 未対応 | n/a | Deploymentスコープ。fraktor-rsでは不要 |

### 11. 構成バリデーション

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `JoinConfigCompatChecker` | `JoinConfigCompatChecker.scala` | 未対応 | easy | Join時の設定互換性チェック |
| `ConfigValidation` (sealed) | `JoinConfigCompatChecker.scala` | 未対応 | easy | Valid / Invalid(errors) |

### 12. 診断・モニタリング

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `ClusterNodeMBean` (JMX) | `ClusterJmx.scala` | 未対応 | n/a | JVM固有 |
| `ClusterLogMarker` | `ClusterLogMarker.scala` | 未対応 | easy | 構造化ログマーカー |
| Metrics（全般） | 散在 | `ClusterMetrics` / `ClusterMetricsSnapshot` | - | 実装済み |

## 設計統合観点（Proto.Actor-go基盤 + Pekko的クラスタ機能）

現行実装は「ClusterProvider + VirtualActor」を既存価値として維持しつつ、以下の2層で拡張すると大規模化の耐性を高められる。

- クラスタ基盤層: `ClusterProvider` がノードの参加/離脱/障害検知を担う。
- 配置統治層: シャード（または仮想Actor配置）を独立制御する層を追加し、`ClusterSharding` 的な責務を担わせる。

この分割にすると、現状は維持しつつ次の順でスコープインできる。

- まず `DowningProvider` を活用して手動downの制御を明確化し、`SplitBrainResolver` 相当の最小自動判定を追加する。
- 次に `Reachability` と `Unreachable/Reachable` 系イベントを追加して、分断耐性と収束品質を上げる。
- 最後に `Placement` 系（leader/role別の制御、コンバージェンス、ルーティング）を追加してシャード統治を高化する。

この見方では、現時点の `proto.actor-go` 基盤は捨てずに、Pekko 的機能を段階的に「差し込む」方式になるため、移行時の破壊的変更を小さく保ちながらスケール要求へ対応できる。

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）
- `Member.appVersion` - NodeRecordにバージョンフィールドを追加するのみ

### Phase 2: easy（単純な新規実装）
- `MemberStatus.Exiting` - NodeStatusにExitingバリアントを追加
- `Member.roles` - NodeRecordにrolesフィールドを追加
- `Member.isOlderThan` - Join順序に基づく比較メソッド
- `Cluster.registerOnMemberUp` / `registerOnMemberRemoved` - EventStreamベースのコールバック
- `JoinConfigCompatChecker` - 設定互換性チェックの基盤trait
- `ClusterSettings.Roles` - ClusterExtensionConfigにロール設定を追加
- `ClusterLogMarker` - 構造化ログマーカー

### Phase 3: medium（中程度の実装工数）
- `Cluster.prepareForFullClusterShutdown` - 全ノード協調シャットダウン
- `LeaderChanged` / `RoleLeaderChanged` イベント + リーダー選出ロジック
- `MemberStatus.WeaklyUp` - Reachability統合が前提
- `MemberStatus.PreparingForShutdown` / `ReadyForShutdown` - 協調シャットダウン
- `VectorClock` - MembershipVersionの分散版（因果順序追跡）
- `Gossip.seen` / `SeenChanged` - コンバージェンス進捗追跡
- `ReachableMember` イベント - Quarantineからの復帰イベント
- `CurrentClusterState` の拡充 - `leader`, `unreachable`, `roleLeaderMap` フィールド追加

### Phase 4: hard（アーキテクチャ変更を伴う）
- `Reachability` モデル - Observer/Subject型の到達可能性追跡
- `SplitBrainResolver` + `DowningStrategy` - ネットワーク分断時のダウニング判定
- `MembershipState.convergence` - 分散コンバージェンス判定
- `ClusterRouterGroupSettings` / `ClusterRouterPoolSettings` - クラスタ対応ルーティング

### 対象外（n/a）
- `ClusterNodeMBean` (JMX) - JVM固有
- `CrossDcClusterHeartbeat` - マルチDC機能は未対応
- `ClusterSettings.SelfDataCenter` - マルチDC関連
- `DataCenterReachabilityChanged` - マルチDC関連
- `ClusterScope` - Deployment スコープ。fraktor-rsの設計では不要
- `GossipOverview` - fraktor-rsはdelta-gossip方式で不要
- Pekko Serialization関連 - JVM Serialization固有
- Aeron UDP transport - JVM固有

## fraktor-rs 独自機能（Pekkoのclusterモジュールに対応がないもの）

以下はfraktor-rsがprotoactor-goから取り入れた機能で、Pekkoでは別モジュールで提供されるもの：

| fraktor-rs | 対応するPekkoモジュール | 備考 |
|-----------|----------------------|------|
| Grain（Virtual Actor）全体 | pekko-cluster-sharding | Entity lifecycle, GrainRef, KindRegistry等 |
| Pub/Sub全体 | pekko-distributed-pub-sub | Topic, Subscriber, BatchingProducer等 |
| PartitionIdentityLookup | pekko-cluster-sharding (ShardRegion) | パーティションベースのID解決 |
| PidCache | pekko-cluster-sharding (内部) | アクティベーション結果のキャッシュ |
| RendezvousHasher | pekko-cluster-sharding (ShardAllocationStrategy) | 一貫性ハッシュによるルーティング |
| PlacementCoordinator | pekko-cluster-sharding (ShardCoordinator) | アクティベーション管理 |
| AwsEcsClusterProvider | pekko-management (外部ライブラリ) | クラウドネイティブディスカバリ |
| OutboundPipeline | pekko-remote (内部) | メッセージパイプライン |
| BlockList | なし | fraktor-rs独自 |

---

## 総評

fraktor-rs の cluster モジュールは **Pekko とは大きく異なる設計方針**（protoactor-go 準拠）を採用しており、Pekko の cluster-sharding や distributed-pub-sub に相当する機能（Grain, Pub/Sub）が統合されている点が特徴的。カバレッジは前回の 58% から **67%** に向上し、`DowningProvider` trait が新たに実装された。

主要なギャップは以下の 3 領域に集中：

1. **到達可能性と分断耐性**（Reachability, SplitBrainResolver）— ネットワーク分断への対処
2. **リーダー選出とコンバージェンス**（Leader, VectorClock, Seen）— 分散合意の強化
3. **クラスタルーティング**（ClusterRouter）— クラスタ対応のメッセージルーティング

YAGNI 原則に従い、Phase 2 の easy 項目（Exiting 状態、ロール、registerOnMemberUp 等）を優先し、SBR 等の hard 項目は本番運用要件が明確になってから検討するのが妥当。なお現状は明示 `down` API を主軸にしており、分断自動判定（SplitBrainResolver 相当）は未対応。

## 次の推奨プラン（全体優先度）

ユーザ要求どおり、当面は `actor` / `streams` を優先し、cluster の拡充は defer する。

- 第1優先: `cluster` は easy 領域で安全に収束  
  - `MemberStatus.Exiting` / `roles` / `isOlderThan` / `registerOnMemberUp` / `registerOnMemberRemoved` / `ClusterLogMarker` などを先に実装
- 第2優先: `SplitBrainResolver` 系は保留  
  - SBR・`Reachability`・`MembershipState.convergence` はクラスタ要件が明確になるまでハイブリッド（明示 `down` 中心）運用を維持
- 第3優先: `actor` / `streams` の Pekko 互換が進んだ後に再評価  
  - `LeaderChanged` / `RoleLeaderChanged` / `ClusterRouter` / Placement 系を同一トランザクションで統合する設計に移行
