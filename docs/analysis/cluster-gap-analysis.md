# cluster モジュール ギャップ分析

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（機能グループ単位） | 78 |
| fraktor-rs 公開型数（core: ~150, std: ~15） | ~165 |
| 機能カバレッジ（機能グループ単位） | 9/14 (64%) |
| 未実装機能グループ数 | 5 |

> **注記**: fraktor-rs は protoactor-go の影響を受けており、Pekko とは設計思想が異なる。
> 特に Grain（仮想アクター）/ Placement / Identity は Pekko の Cluster Sharding に相当する独自実装であり、
> 型数が多いのはこの部分の充実による。Pekko の全機能を移植することが目的ではない（YAGNI原則）。

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core（コアロジック） | 78 | ~150 | 充実（独自拡張含む） |
| std（アダプタ） | N/A（Pekko は JVM 単一層） | ~15 | 該当機能は実装済み |

> fraktor-rs は core/std 分離により Pekko より型数が増える傾向がある（Shared ラッパー、エラー型等）。

## 機能グループ別マッピングと対応状況

### 1. コアクラスタ（メンバーシップ・ライフサイクル・イベント） ✅ 実装済み 16/16 (100%)

| Pekko 概念 | fraktor-rs 対応 | 備考 |
|-----------|----------------|------|
| `Cluster` (extension) | `ClusterExtension` | 同等機能 |
| `Member` | `NodeRecord` | 別名で実装 |
| `MemberStatus` | `NodeStatus` | 別名で実装 |
| `UniqueAddress` | authority 文字列 | 簡略化された実装 |
| `CurrentClusterState` | `CurrentClusterState` | 同名 |
| `ClusterEvent` hierarchy | `ClusterEvent` + `ClusterEventType` | 同等 |
| `ClusterSettings` | `ClusterExtensionConfig` | 同等 |
| `ClusterReadView` | `current_cluster_state_snapshot()` | メソッドとして実装 |

ギャップなし。

### 2. ゴシッププロトコル ✅ 実装済み 4/4 (100%)

| Pekko 概念 | fraktor-rs 対応 | 備考 |
|-----------|----------------|------|
| `Gossip` | `GossipDisseminationCoordinator` | 同等 |
| `GossipOverview` | `GossipState` | 同等 |
| `VectorClock` | `VectorClock` | 同名 |
| `Reachability` | `MembershipTable` の NodeStatus 管理 | 統合実装 |

ギャップなし。

### 3. ハートビート・障害検知 ⚠️ 部分実装 0/2 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ClusterHeartbeat` | `ClusterHeartbeat.scala` | 部分実装 | core | medium | MembershipCoordinator の poll() で heartbeat miss を検知。専用のハートビートアクターはない |
| `CrossDcClusterHeartbeat` | `CrossDcClusterHeartbeat.scala` | 未対応 | core | hard | マルチDC対応が前提 |

### 4. シードノードプロセス ⚠️ 部分実装 0/1 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `SeedNodeProcess` | `SeedNodeProcess.scala` | 部分実装 | std | medium | `LocalClusterProvider.seed_nodes()` で設定可能だが、自動的なシードノード発見プロセスはない |

### 5. 設定互換性チェック ✅ 実装済み 2/2 (100%)

| Pekko 概念 | fraktor-rs 対応 | 備考 |
|-----------|----------------|------|
| `JoinConfigCompatChecker` | `JoinConfigCompatChecker` | 同名 |
| `ConfigValidation` | `ConfigValidation` | 同名 |

ギャップなし。

### 6. ダウニング・Split Brain Resolver ⚠️ 部分実装 2/7 (29%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `DowningProvider` | `DowningProvider.scala` | 実装済み | core | - | trait として定義済み |
| `NoDowning` | `DowningProvider.scala` | 実装済み | core | - | `NoopDowningProvider` として実装 |
| `SplitBrainResolver` | `sbr/SplitBrainResolver.scala` | 未対応 | core | hard | ネットワーク分断対処の主要戦略 |
| `DowningStrategy` | `sbr/DowningStrategy.scala` | 未対応 | core | hard | 戦略プラグイン基盤 |
| `SplitBrainResolverSettings` | `sbr/SplitBrainResolverSettings.scala` | 未対応 | core | easy | SBR 実装に付随 |
| `SplitBrainResolverProvider` | `sbr/SplitBrainResolverProvider.scala` | 未対応 | core | easy | SBR 実装に付随 |
| `Decision` types | `sbr/DowningStrategy.scala` | 未対応 | core | easy | SBR 実装に付随 |

### 7. Cluster Typed API ❌ 未実装 0/5 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Cluster` (typed) | `cluster-typed/Cluster.scala` | 未対応 | core/typed | medium | typed ラッパー |
| `ClusterCommand` | `cluster-typed/Cluster.scala` | 未対応 | core/typed | easy | sealed trait + case classes |
| `ClusterStateSubscription` | `cluster-typed/Cluster.scala` | 未対応 | core/typed | easy | サブスクリプション管理 |
| `SelfUp` / `SelfRemoved` | `cluster-typed/Cluster.scala` | 未対応 | core/typed | trivial | typed 固有イベント |

### 8. クラスタルーティング ✅ 実装済み 4/4 (100%)

| Pekko 概念 | fraktor-rs 対応 | 備考 |
|-----------|----------------|------|
| `ClusterRouterConfig` | `ClusterRouterPool` + `ClusterRouterGroup` | 分離設計 |
| `ClusterRouterSettings` | `ClusterRouterPoolSettings` + `ClusterRouterGroupSettings` | 同等 |

ギャップなし。

### 9. クラスタシングルトン ❌ 未実装 0/5 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ClusterSingleton` | `cluster-typed/ClusterSingleton.scala` | 未対応 | core | hard | クラスタ全体で1つのアクターを保証する仕組み |
| `SingletonActor[M]` | `cluster-typed/ClusterSingleton.scala` | 未対応 | core | medium | シングルトン設定ラッパー |
| `ClusterSingletonSettings` | `cluster-typed/ClusterSingleton.scala` | 未対応 | core | easy | 設定型 |
| `ClusterSingletonManager` | `cluster-tools/singleton/ClusterSingletonManager.scala` | 未対応 | core | hard | ライフサイクル管理 |
| `ClusterSingletonProxy` | `cluster-tools/singleton/ClusterSingletonProxy.scala` | 未対応 | core | medium | シングルトンへのプロキシ |

### 10. クラスタシャーディング ✅ 同等実装（protoactor-go スタイル） 10/10 (100%)

fraktor-rs は Pekko の Cluster Sharding に相当する機能を、protoactor-go の Grain/Placement/Identity として実装している。

| Pekko 概念 | fraktor-rs 対応 | 備考 |
|-----------|----------------|------|
| `ClusterSharding` | `ClusterExtension` + `PlacementCoordinator` | 統合 |
| `ShardRegion` | `VirtualActorRegistry` | 同等 |
| `ShardCoordinator` | `PlacementCoordinator` | 同等 |
| `Entity[M]` | `ActivatedKind` | 同等 |
| `EntityTypeKey[M]` | `GrainKey` | 同等 |
| `EntityRef[M]` | `GrainRef` | 同等 |
| `ShardingEnvelope[M]` | `SerializedMessage` | 同等 |
| `ClusterShardingSettings` | `ClusterExtensionConfig` | 統合 |
| `ShardedDaemonProcess` | （該当なし） | protoactor-go にもない概念 |
| `MessageExtractor` | `SchemaNegotiator` + `GrainCodec` | 異なるアプローチ |

> **注記**: fraktor-rs の Grain/Placement/Identity は Pekko の Sharding より細粒度な設計。
> `IdentityLookup`, `PartitionIdentityLookup`, `RendezvousHasher`, `PidCache` 等のコンポーネントが
> Pekko の内部実装に相当する機能を公開 API として提供している。

### 11. 分散 PubSub ✅ 実装済み 3/3 (100%)

| Pekko 概念 | fraktor-rs 対応 | 備考 |
|-----------|----------------|------|
| `DistributedPubSub` | `ClusterPubSub` + `PubSubApi` | 同等以上 |
| `DistributedPubSubMediator` | `PubSubBroker` | 同等 |
| `DistributedPubSubSettings` | `PubSubConfig` + `PubSubTopicOptions` | 同等 |

> fraktor-rs は Pekko より豊富な PubSub 機能を持つ（`BatchingProducer`, `DeliveryPolicy`,
> `DispatchDropPolicy`, `PartitionBehavior`, `PublishOptions` 等）。

### 12. クラスタメトリクス ⚠️ 部分実装 1/4 (25%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ClusterMetrics` (extension) | `cluster-metrics/ClusterMetrics.scala` | 部分実装 | core | medium | `ClusterMetrics` + `ClusterMetricsSnapshot` があるが、Pekko のような subscribe/unsubscribe メカニズムはない |
| `NodeMetrics` | `cluster-metrics/NodeMetrics.scala` | 未対応 | core | medium | ノード単位のメトリクス収集 |
| `Metric` | `cluster-metrics/NodeMetrics.scala` | 未対応 | core | easy | 個別メトリクス値 |
| `ClusterMetricsEvent` | `cluster-metrics/ClusterMetrics.scala` | 未対応 | core | easy | メトリクス変更イベント |

### 13. 分散データ（CRDT） ❌ 未実装 0/14 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Replicator` | `ddata/Replicator.scala` | 未対応 | core | hard | CRDT レプリケーション基盤 |
| `ReplicatedData` trait | `ddata/ReplicatedData.scala` | 未対応 | core | medium | CRDT 基底 trait |
| `GCounter` | `ddata/GCounter.scala` | 未対応 | core | easy | 増加のみカウンタ |
| `PNCounter` | `ddata/PNCounter.scala` | 未対応 | core | easy | 増減カウンタ |
| `LWWRegister[T]` | `ddata/LWWRegister.scala` | 未対応 | core | easy | Last-Writer-Wins レジスタ |
| `ORSet[A]` | `ddata/ORSet.scala` | 未対応 | core | medium | Observed-Remove セット |
| `ORMap[K,V]` | `ddata/ORMap.scala` | 未対応 | core | medium | Observed-Remove マップ |
| `LWWMap[K,V]` | `ddata/LWWMap.scala` | 未対応 | core | medium | LWW マップ |
| `PNCounterMap[K]` | `ddata/PNCounterMap.scala` | 未対応 | core | easy | キー別 PNCounter |
| `ORMultiMap[K,V]` | `ddata/ORMultiMap.scala` | 未対応 | core | medium | マルチバリューマップ |
| `Flag` | `ddata/Flag.scala` | 未対応 | core | trivial | ブール CRDT |
| `Key[T]` | `ddata/Key.scala` | 未対応 | core | easy | 型安全キー |
| `ReadConsistency` / `WriteConsistency` | `ddata/Replicator.scala` | 未対応 | core | easy | 整合性レベル |
| `DistributedData` (typed) | `ddata/typed/` | 未対応 | core/typed | medium | Typed API |

### 14. Coordinated Shutdown ❌ 未実装 0/1 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `CoordinatedShutdownLeave` | `CoordinatedShutdownLeave.scala` | 未対応 | std | medium | グレースフルシャットダウンとの統合 |

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）

- `SelfUp` / `SelfRemoved` イベント（core/typed）— Cluster Typed の部分的導入
- `Flag` CRDT（core）— 最小の CRDT 実装

### Phase 2: easy（単純な新規実装）

- `SplitBrainResolverSettings` / `SplitBrainResolverProvider`（core）— SBR 基盤の設定型
- `ClusterCommand` sealed enum（core/typed）— Typed API の入口
- `GCounter` / `PNCounter`（core）— 基本的な CRDT
- `LWWRegister`（core）— シンプルな CRDT
- `Key[T]` / `ReadConsistency` / `WriteConsistency`（core）— CRDT 周辺型
- `Metric` / `ClusterMetricsEvent`（core）— メトリクス拡張

### Phase 3: medium（中程度の実装工数）

- `SeedNodeProcess`（std）— 自動シードノード発見
- `NodeMetrics`（core）— ノード単位メトリクス
- `ClusterSingletonSettings` / `SingletonActor`（core）— シングルトン設定
- `ClusterSingletonProxy`（core）— シングルトンプロキシ
- `ORSet` / `ORMap` / `LWWMap`（core）— 主要 CRDT
- `ReplicatedData` trait（core）— CRDT 基底
- `Cluster` typed wrapper（core/typed）— Typed API メイン
- `CoordinatedShutdownLeave`（std）— グレースフルシャットダウン統合

### Phase 4: hard（アーキテクチャ変更を伴う）

- `SplitBrainResolver` / `DowningStrategy`（core）— ネットワーク分断対処。MembershipCoordinator との統合が必要
- `ClusterSingleton` / `ClusterSingletonManager`（core）— クラスタ全体のリーダー選出とアクター管理。PlacementCoordinator との関係を整理する必要あり
- `Replicator`（core）— CRDT レプリケーション基盤。Gossip プロトコルとの統合が必要
- `CrossDcClusterHeartbeat`（core）— マルチデータセンター対応。アーキテクチャレベルの設計が必要

### 対象外（n/a）

| Pekko API | 理由 |
|-----------|------|
| `ClusterJmx` | JVM 固有（JMX モニタリング） |
| `ClusterLogClass` / `ClusterLogMarker` | JVM ロギングフレームワーク固有 |
| `ClusterDaemon` | Pekko 内部実装（`private[cluster]`） |
| `ClusterActorRefProvider` | Pekko 内部のプロバイダメカニズム |
| `ShardingFlightRecorder` / `ShardingLogMarker` | JVM 診断固有 |
| `OldCoordinatorStateMigrationEventAdapter` | Akka → Pekko 移行用 |
| `ShardedDaemonProcess` | protoactor-go にもない概念、YAGNI |
| `DurableStore` | CRDT 永続化（Replicator 実装後に検討） |

## まとめ

### 全体カバレッジ評価

fraktor-rs の cluster モジュールは **コアとなるメンバーシップ管理、ゴシッププロトコル、仮想アクター（Grain/Sharding相当）、PubSub、ルーティングは十分にカバー**されている。特に PubSub と Grain（仮想アクター）は Pekko より細粒度で豊富な API を提供しており、protoactor-go の影響を受けた独自の強みとなっている。

### 即座に価値を提供できる未実装機能（Phase 1〜2）

- **基本的な CRDT 型**（`GCounter`, `PNCounter`, `Flag`）: 分散システムでの状態共有に直結
- **Cluster Typed API の部分導入**（`ClusterCommand`, `SelfUp`/`SelfRemoved`）: 型安全な API

### 実用上の主要ギャップ（Phase 3〜4）

- **Split Brain Resolver**: プロダクション環境で最も重要な未実装機能。ネットワーク分断時のクラスタ安定性に直結する。現状 `NoopDowningProvider` のみ
- **Cluster Singleton**: クラスタ全体で1つのアクターを保証する機能。リーダー選出やスケジューラ等のユースケースで必要
- **Distributed Data (CRDT)**: 分散状態管理の基盤。Replicator + 主要 CRDT 型の実装は大きな工数だが、分散システムの根幹機能

### YAGNI 観点での省略推奨

- **CrossDcClusterHeartbeat**: マルチDC対応は現時点で不要（要件が明確化してから検討）
- **ShardedDaemonProcess**: Grain パターンでカバー可能
- **ORMultiMap / PNCounterMap**: 基本 CRDT 実装後、要件に応じて追加
- **Cluster Typed API 全体**: fraktor-rs の actor モジュールに typed 層が確立されてから検討すべき。先に actor の typed 層を整備する必要がある
