# cluster モジュール ギャップ分析

更新日: 2026-06-11 (parity 分母再構成 + 全カテゴリ再検証)

## 位置づけ

fraktor-rs の `cluster-*` は、Proto.Actor-Go 型の Virtual Actor / Grain runtime を主軸にする。直近の実装順序は [2026-05-25_cluster-grain-runtime-roadmap.md](../plan/2026-05-25_cluster-grain-runtime-roadmap.md) と個別の OpenSpec change が管理する。

一方、この文書は Pekko 比較スコープにおける **parity 完了のための全量計画** を提示する。実装優先度（Phase 1〜3）には YAGNI を適用せず、未要求であっても parity ギャップである限り順位付けの対象に含める。各 Phase の項目をどの時点で着手するかは roadmap / OpenSpec 側の判断であり、この文書はギャップの「漏れのない台帳」と「埋める順序」を提供する。

## 比較スコープ定義

cluster membership と virtual actor / sharding 相当の分散配置契約を対象にし、JVM 実装技術、Java/Scala DSL convenience、testkit、`cluster-metrics` は parity 分母から除外する。Pekko submodule は `2dc8960074bfe269da1686609eb88663cb50ad8b` を参照した。

### 対象に含めるもの

| 領域 | fraktor-rs | Pekko 参照 |
|------|------------|------------|
| cluster core | `modules/cluster-core-kernel/src/` (`activation`, `cluster_provider`, `downing_provider`, `extension`, `failure_detector`, `grain`, `membership`, `message_serialization`, `outbound`, `pub_sub`, `topology`) | `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/` |
| typed cluster contract | `modules/cluster-core-typed/src/` | `references/pekko/cluster-typed/src/main/scala/` |
| sharding / virtual actor | `modules/cluster-core-kernel/src/grain/`, `modules/cluster-core-kernel/src/activation/` | `references/pekko/cluster-sharding/`, `references/pekko/cluster-sharding-typed/` |
| cluster tools | `modules/cluster-core-kernel/src/pub_sub/` | `references/pekko/cluster-tools/src/main/scala/org/apache/pekko/cluster/pubsub/`, `singleton/` |
| distributed data | 対応モジュールなし | `references/pekko/distributed-data/src/main/scala/org/apache/pekko/cluster/ddata/` |
| std adapter | `modules/cluster-adaptor-std/src/` | gossip transport / provider / discovery adapter として Rust で再現可能な契約 |

### 対象から除外するもの

| 除外項目 | 理由 |
|----------|------|
| `cluster-metrics` | スキル定義で明示的に別スコープ |
| `ClusterClient` / `ClusterClientReceptionist` / `ClusterClientSettings` / `ClusterReceptionistSettings` | Pekko 本体で全面 `@deprecated`（gRPC 移行推奨）。deprecated は n/a 対象 |
| `@InternalApi` / `private[...]` の型（SBR 内部 `DowningStrategy` 実装、typed `ClusterReceptionist`、Replicator 内部プロトコル等） | 公開契約ではない |
| Kubernetes / discovery backend 固有実装の完全互換 | backend 実装技術ごとの互換は std adapter の別調査対象 |
| multi-node-testkit / cluster tests / typed tests / `TestEntityRef` | runtime API ではない |
| Java DSL / Scala DSL convenience / implicit syntax | Rust API として再現する対象ではない |
| JVM management / JMX / HOCON dynamic loading / classloader | JVM 固有 |
| protobuf serializer の完全バイナリ互換 | contract 接続は対象だが、JVM serializer 実装そのものは対象外 |
| JFR / log marker の JVM 固有 event class | Rust 側は tracing / event stream contract として扱う（構造化ログマーカー契約自体はカテゴリ1で対象） |
| `RemoveInternalClusterShardingData` | JVM persistence データ migration utility |

### 分母再構成（2026-06-11）

旧版の分母「約 121 概念」を、公開契約単位で再検証して **151 概念** に再構成した。主な変更:

- ClusterClient 系 4 概念を deprecated として n/a へ移動（カテゴリ6 から除外）
- SBR の `DowningStrategy` 実装クラス・`SplitBrainResolverSettings` は全て `@InternalApi private[sbr]` のため分母から除外（カテゴリ3 を 8 → 5 へ縮小）
- typed API のコマンド・購読・イベント型を概念単位で明示（カテゴリ5 を 7 → 14 へ拡大）
- sharding の query protocol、health check、passivation strategy settings、allocation SPI、settings 契約の漏れを追加（カテゴリ8 を 19 → 27 へ拡大）
- ddata の CRDT 全種・Replicator プロトコル・durable store・typed adapter を概念単位で明示（カテゴリ9 を 18 → 27 へ拡大）
- membership / lifecycle に ClusterSettings 契約、Member ordering、CoordinatedShutdownLeave、ClusterLogMarker、ClusterScope、Multi-DC 設定を追加（カテゴリ1 を 17 → 22 へ拡大）

### raw 抽出値の扱い

fraktor-rs 側はスキル指定の `pub` 系抽出で、型 297 件 (core-kernel: 265, core-typed: 9, std: 23)、公開メソッド 843 件 (core-kernel: 730, core-typed: 32, std: 81)。この数には `pub(crate)` の helper も含まれる参考値であり、parity カバレッジ分母には使わない。Pekko 側 raw 抽出（型宣言 857 件 / 主要 `def` 3027 件）も同様に参考値。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 151 |
| fraktor-rs 固定スコープ対応概念（実装済み） | 87 |
| 固定スコープ概念カバレッジ | 87/151 (58%) |
| 部分実装 | 12 |
| 未対応 | 52 |
| raw public type declarations | 297 (core-kernel: 265, core-typed: 9, std: 23) |
| raw public method declarations | 843 (core-kernel: 730, core-typed: 32, std: 81) |
| hard / medium / easy / trivial gap | 11 / 26 / 17 / 3 |
| panic 系スタブ | 0 件 |
| 機能 placeholder / TODO | 0 件 |

注: `raw public` は `pub(crate)` など内部到達可能な `pub` を含む参考値であり、crate 外から到達可能な外部公開 API 数ではない。

### 前回 (2026-06-05) からの判定変更

| 項目 | 旧判定 | 新判定 | 根拠 |
|------|--------|--------|------|
| std `ClusterApi` wrapper parity | 部分実装（get/request/down のみ） | 実装済み | `ClusterApi`（core-kernel）に `join` / `leave` / `subscribe` / `subscribe_no_replay` / `unsubscribe` / `current_state` / `self_authority` / `remote_path_of` / `down` / `get` / `request` / `request_future` がフルセットで存在。std は core API を直接使う設計で独自 wrapper は不要 |
| membership-driven routee add/remove | 実装済み (core policy) | 部分実装 | core policy（`ClusterRouterPool::update_from_members`）は完了だが、ClusterEvent 購読 → routee 更新の std 配線が未実装で end-to-end では動かない |
| `SubscriptionInitialStateMode` | 分母外 | 実装済み | `ClusterSubscriptionInitialStateMode` が extension モジュールに存在 |
| pub-sub query | mediator protocol に包含 | 概念分離して実装済み/部分実装 | `MediatorQuery::CurrentTopics` / `SubscriberCount` は実装済み。mediator 全体の `Count`（トピック横断 subscriber 総数）のみ未対応 |
| `MembershipCoordinatorDriver` | 実装済み概念として列挙 | 公開型ではない | `pub(super)`。`TokioGossiper` の内部実装であり外部 API には露出しない |

備考: `ClusterPubSub` trait は `pub_sub::cluster_pub_sub::ClusterPubSub` のネストパスでのみ公開されており、トップレベル `pub_sub` への re-export はない（実装済み判定は維持、公開面の整理は別件）。`NodeStatus` の Pekko `Down` 相当は `Dead` バリアント（別名実装済み）。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core / membership | `Cluster`, `Member`, `MemberStatus`, `CurrentClusterState`, `ClusterEvent`, `Gossip`, `Reachability` | `ClusterExtension`, `ClusterApi`, `NodeRecord`, `NodeStatus`, `CurrentClusterState`, `MembershipCoordinator`, `GossipDisseminationCoordinator`, `ReachabilityMatrix`, `GossipStateModel`, `HeartbeatProtocolState`, `CrossDcHeartbeat` | UniqueAddress、data center、WeaklyUp、reachability matrix、gossip envelope、full gossip merge / tombstone / seen digest、dedicated heartbeat evidence、SeedNodeProcess の core contract はある。Member ordering 公開契約、shutdown 系イベント型、CoordinatedShutdown 連携が不足 |
| core / downing | `DowningProvider`, `NoDowning`, `SplitBrainResolverProvider` | `DowningProvider`, `DowningDecisionContext`, `DowningStrategyDecision`, `DowningDecisionTrace`, `NoopDowningProvider`, `SplitBrainResolver`, `LeaseMajorityPort`, `SplitBrainResolverProviderHook` | decision model / settings / provider binding は完了。SBR runtime actor（reachability 変化監視 → 責任ノード選択 → 自動 down 発行ループ）と concrete lease backend が未実装 |
| core / typed | typed `Cluster`, command, subscription, singleton, sharding typed API | `Cluster` / `ClusterCommand` / `ClusterStateSubscription` / `ClusterEventSubscription` / `SelfUp` / `SelfRemoved` / `ClusterSetup` | typed Cluster facade（subscribe / unsubscribe / current_state 含む）はほぼ完備。`PrepareForFullClusterShutdown` command と singleton / sharding typed API が未実装 |
| core / virtual actor | `ClusterSharding`, `EntityRef`, `EntityTypeKey`, `ShardRegion`, coordinator | `GrainRef`, `GrainKey`, `VirtualActorRegistry`, `PlacementCoordinatorCore`, `PartitionIdentityLookup`, `RendezvousHasher`, `PidCache` | protoactor-go style の同等機能は強いが、Pekko public API 形態（typed Entity/EntityRef/Extractor）、rebalance / remembered entities / query protocol / health check が不足 |
| core / distributed state | `DistributedData`, `Replicator`, CRDT 型群 | なし | 全面未実装 |
| std / adapter | gossip transport, provider, discovery adapter | `TokioGossipTransport`, `TokioGossiper`, `LocalClusterProvider`, `StaticClusterProvider`, `AwsEcsClusterProvider`, `GenericDiscoveryAdapter`, `ProviderLifecycleBridge`, `ClusterWireCodec`, `ConfiguredPhiAccrualDetectorFactory` | Rust adapter、logical envelope handoff、seed/discovery provider boundary、cluster message serializer contract、failure detector factory はある。sharding/singleton 系 setup と sharding join compat key が不足 |

## カテゴリ別ギャップ

各カテゴリのヘッダーに **実装済み数 / 対象概念数 (カバレッジ%)** を明記する。ギャップ（未対応・部分実装）のみテーブルに列挙し、実装済みは件数カウントに含めてテーブル行には追加しない。

### 1. Cluster membership / lifecycle　✅ 実装済み 18/22 (82%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `prepareForFullClusterShutdown` | `Cluster.scala:336` | 部分実装 | core + std | medium | `NodeStatus::PreparingForShutdown` / `ReadyForShutdown` と状態遷移・進行イベントは実装済みだが、`ClusterApi` / `ClusterCommand` から起動する full shutdown command path がない |
| `CoordinatedShutdownLeave` | `CoordinatedShutdownLeave.scala:30` | 未対応 | core/extension + std | medium | coordinated shutdown phase から cluster leave を駆動する hook がない（actor-core 側に CoordinatedShutdown はある） |
| `ClusterScope` deploy scope | `ClusterActorRefProvider.scala:148` | 未対応 | core/extension | medium | cluster-aware deployment scope 概念がない。router config はあるが deploy scope としての統合はない |
| `ClusterSettings.CrossDcFailureDetectorSettings` / `MultiDataCenter` | `ClusterSettings.scala:65`, `ClusterSettings.scala:76` | 未対応 | core/config | easy | `CrossDcHeartbeat` evidence と `FailureDetectorConfig` はあるが、Multi-DC 専用の failure detector 設定 namespace がない |

実装済みとして扱うもの: cluster extension、join/leave/down（`ClusterApi` フルセット）、event stream subscription、current state snapshot、member/up/removed callback、roles/app_version 設定、leader/role leader 算出、startup/shutdown event、`UniqueAddress` semantics（`NodeRecord::unique_address` / `try_join_with_identity`）、data center membership、`WeaklyUp`、`remotePathOf`、`MemberStatus` 全 variant（`Down` ≈ `Dead` 別名実装済み）、`PreparingForShutdown` / `ReadyForShutdown` status、`ClusterSettings` 契約（`ClusterExtensionConfig` + `FailureDetectorConfig` + `ConfigValidation`）、`JoinConfigCompatChecker` + `ConfigValidation`、Member ordering 公開契約（`member_age_order` / `age_ordered` / `oldest_member`、2026-06-11 cluster-membership-event-surface）、`ClusterLogMarker` 相当の構造化 tracing field 契約（`cluster_lifecycle_trace_field` + std `ClusterLifecycleLogSubscriber`、同上）。

### 2. Gossip / reachability / failure detection　✅ 実装済み 18/18 (100%)

このカテゴリの未対応ギャップは解消済み（2026-06-11 cluster-membership-event-surface で `MemberPreparingForShutdown` / `MemberReadyForShutdown` イベント variant + coordinator 併発、`UnreachableDataCenter` / `ReachableDataCenter` イベント + `DataCenterReachabilityTable` ラッチを実装）。

実装済みとして扱うもの: `Reachability` matrix（`ReachabilityMatrix` / `ReachabilityRecord` / `ReachabilitySnapshot`）、full `Gossip` merge / tombstone / seen digest（`GossipStateModel` / `GossipStateSnapshot`）、`GossipEnvelope` + logical handoff、dedicated heartbeat protocol（`HeartbeatProtocolState`）、`CrossDcClusterHeartbeat` evidence（`CrossDcHeartbeat`）、`SeedNodeProcess`、config compatibility full key set（`ClusterCompatibilityKeyCatalog` / `JoinCompatibilityComposition`）、Failure Detector Configuration（`FailureDetectorConfig` + `ConfiguredPhiAccrualDetectorFactory`）、`SubscriptionInitialStateMode`（`ClusterSubscriptionInitialStateMode`）、`MembershipTable` / `MembershipDelta` / `MembershipVersion` / `VectorClock`、`DefaultFailureDetectorRegistry`、`MembershipCoordinator::poll` による suspect/dead 遷移、indirect connection evidence、`TokioGossipTransport`。

### 3. Downing / Split Brain Resolver　✅ 実装済み 3/5 (60%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| SBR runtime down execution loop | `SplitBrainResolver.scala:160`（@InternalApi actor だが機能として公開契約） | 部分実装 | core + std | hard | `SplitBrainResolver` の decision model / trace / provider hook は実装済み。reachability 変化の購読、stable-after タイマー駆動、責任ノード選択、`ClusterCore::down` への自動発行ループが未実装。現状 down は明示呼び出しのみ |
| concrete lease coordination backend | `DowningStrategy.scala:602` | 部分実装 | std | hard | `LeaseMajorityPort` / `LeaseAcquisitionOutcome` / `StdLeaseMajorityBackend` trait は実装済み。実際の分散 lease backend（coordination service 連携、retry、network I/O）が未実装 |

実装済みとして扱うもの: `DowningProvider` SPI、`NoDowning`（`NoopDowningProvider`）、SBR settings 契約（`SplitBrainResolverConfig` / `SplitBrainResolverStrategy`: KeepMajority / StaticQuorum / KeepOldest / DownAll / LeaseMajority の 5 戦略）。`DowningDecisionContext` / `DowningStrategyDecision` / `DowningDecisionTrace` / `FailureObservation` / `IndirectConnectionEvidence` / `SplitBrainResolverProviderHook` / `StdSplitBrainResolverProvider` / downing provider・SBR settings の join compatibility は上記概念の evidence。

### 4. Cluster router pool / group　✅ 実装済み 5/6 (83%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| membership-driven routee add/remove | `ClusterRouterConfig.scala:586` | 部分実装 | std | easy | core policy（`ClusterRouterPool::update_from_members` / group `local_routee_paths`）は実装済み。ClusterEvent 購読 → routee 更新の std 配線が未実装（判定変更: 旧「実装済み」） |

実装済みとして扱うもの: `ClusterRouterPool`、`ClusterRouterGroup`、pool/group settings の分離、`use_roles` / `satisfies_roles`、`max_instances_per_node`、`ClusterRouterPool::from_candidates` の least-loaded 配置。

### 5. Cluster Typed API　✅ 実装済み 13/14 (93%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `PrepareForFullClusterShutdown` command | `cluster-typed/Cluster.scala:175` | 未対応 | core/typed + std | medium | typed `ClusterCommand` に variant がなく、kernel 側 command path（カテゴリ1）も未実装。coordinated shutdown 接続が必要 |

実装済みとして扱うもの: typed `Cluster` facade、`ClusterStateSubscription` 契約、Subscribe（`Cluster::subscribe` / `subscribe_self_up` / `subscribe_self_removed`）、Unsubscribe（`Cluster::unsubscribe`）、GetCurrentState（`Cluster::current_state`）、`ClusterCommand` sealed 契約と Join / JoinSeedNodes / Leave / Down、`SelfUp`、`SelfRemoved`、typed Cluster extension（`Cluster::get`）、`ClusterSetup`。

### 6. Cluster singleton　✅ 実装済み 0/10 (0%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| classic `ClusterSingletonManager` | `ClusterSingletonManager.scala:492` | 未対応 | std + core | hard | oldest-node election、handover protocol、termination message が必要 |
| `ClusterSingletonManagerSettings` (classic) | `ClusterSingletonManager.scala:55` | 未対応 | core/config | easy | role / removal margin / handover retry 設定型がない |
| `ClusterSingletonProxy` | `ClusterSingletonProxy.scala:171` | 未対応 | std + core | medium | singleton location 追跡と proxy 送信 / buffering がない |
| `ClusterSingletonProxySettings` | `ClusterSingletonProxy.scala:37` | 未対応 | core/config | easy | proxy 設定型がない |
| typed `ClusterSingleton` extension | `cluster-typed/ClusterSingleton.scala:135` | 未対応 | core/typed + std | hard | cluster 全体で一つの actor を保証する typed extension がない |
| typed `ClusterSingletonSettings` | `cluster-typed/ClusterSingleton.scala:32` | 未対応 | core/config | easy | typed 設定型がない |
| `SingletonActor[M]` | `cluster-typed/ClusterSingleton.scala:153` | 未対応 | core/typed | medium | singleton entity 設定 wrapper がない |
| typed `ClusterSingletonManagerSettings` | `cluster-typed/ClusterSingleton.scala:223` | 未対応 | core/config | easy | typed manager 設定型がない |
| `ClusterSingletonSetup` | `cluster-typed/ClusterSingleton.scala:326` | 未対応 | core/typed + std | easy | ActorSystem setup 統合がない |
| `ClusterSingletonManagerIsStuck` 検知契約 | `ClusterSingletonManager.scala`（exception/failure 契約） | 未対応 | core | easy | handover stuck 検知と通知の契約がない |

n/a へ移動: `ClusterClient` / `ClusterClientReceptionist` / `ClusterClientSettings` / `ClusterReceptionistSettings`（Pekko 本体で全面 `@deprecated`、gRPC 移行推奨）。typed `ClusterReceptionist` は `@InternalApi`（receptionist の公開契約は actor-typed 側スコープ）。

### 7. Distributed PubSub　✅ 実装済み 10/11 (91%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| mediator 全体 `Count` query | `DistributedPubSubMediator.scala:253` | 部分実装 | core/pub_sub | trivial | `MediatorQuery::SubscriberCount`（topic 単位）はあるが、トピック横断の subscriber 総数 query がない |

実装済みとして扱うもの: `DistributedPubSubMediator` protocol（`MediatorCommand` / `MediatorAcknowledgement` / `DistributedPubSubMediatorState`）、`DistributedPubSubConfig`、topic registry gossip / delta collection（`TopicRegistryStatus` / `TopicRegistryDelta` / `TopicRegistryDeltaCollector` / `PubSubGossipHandoff`）、`Send` / `SendToAll` path semantics（`MediatorPathKey` / `PubSubPathSemantics`）、`DistributedPubSub` extension 相当（`ClusterPubSub` trait / `ClusterPubSubImpl` / `ClusterPubSubShared`）、Subscribe / Unsubscribe / Publish メッセージ、`GetTopics` / `CurrentTopics`（`MediatorQuery::CurrentTopics`）、`CountSubscribers`（`MediatorQuery::SubscriberCount`）、`DistributedPubSubMessage` marker 相当（`ClusterMessagePayloadKind` / `PubSubEnvelope`）、broker / delivery（`PubSubBroker` + std `PubSubDeliveryActor` / `PubSubDeliveryIntentExecutor`）。

### 8. Sharding / Grain / Placement / Identity　✅ 実装済み 11/27 (41%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| classic `ClusterSharding.start/startProxy` API | `ClusterSharding.scala:224`, `ClusterSharding.scala:516` | 部分実装 | core/grain + std | medium | `setup_member_kinds` / `GrainRef` はあるが Pekko 風 start/startProxy API（proxy-only mode 含む）はない |
| typed `ClusterSharding` extension | `typed/scaladsl/ClusterSharding.scala:40` | 部分実装 | core/typed | medium | grain API はあるが typed extension 形態の `init(Entity)` API ではない |
| `Entity[M, E]` / `EntityContext` | `typed/scaladsl/ClusterSharding.scala:238`, `typed/scaladsl/ClusterSharding.scala:363` | 部分実装 | core/typed + grain | medium | `ActivatedKind` / `GrainContext` は対応するが typed behavior factory ではない |
| `EntityTypeKey[M]` / typed `EntityRef[M]`（ask / askWithStatus 含む） | `typed/scaladsl/ClusterSharding.scala:407`, `typed/scaladsl/ClusterSharding.scala:429` | 部分実装 | core/typed + grain | easy | `GrainKey` / `GrainRef`（`request` / `request_future` が ask 相当）はあるが、message 型でパラメータ化された typed key/ref wrapper がない |
| `ShardingEnvelope` / `ShardingMessageExtractor` SPI | `ShardingMessageExtractor.scala:21`, `typed/ClusterSharding.scala:124` | 部分実装 | core/grain | medium | `SerializedMessage` / `GrainCodec` はあるが envelope / extractor の SPI 契約がない |
| shard allocation / rebalance strategy | `ShardCoordinator.scala:110`, `ShardCoordinator.scala:295` | 部分実装 | core/placement | hard | rendezvous hashing による placement はあるが、`ShardAllocationStrategy` SPI、`LeastShardAllocationStrategy` 相当の rebalance、coordinator handoff protocol がない |
| `ClusterShardingSettings`（classic + typed） | `ClusterShardingSettings.scala:32`, `typed/ClusterShardingSettings.scala:33` | 部分実装 | core/config | medium | `GrainCallOptions` / `PartitionIdentityLookupConfig` 等の個別設定はあるが、包括的な sharding settings 契約がない |
| sharding query protocol（`GetShardRegionState` / `GetShardRegionStats` / `GetClusterShardingStats` / `GetCurrentRegions` + 応答型、classic + typed） | `ShardRegion.scala:237-386`, `ClusterShardingQuery.scala:39` | 未対応 | core/grain + core/typed | medium | shard / region / entity 数の observability query がない（`GrainMetrics` は別系統の metrics） |
| `ClusterShardingHealthCheck` | `ClusterShardingHealthCheck.scala:46` | 未対応 | std | easy | region 登録状態に基づく readiness check がない |
| passivation strategy settings（idle / LRU / MRU / LFU / admission） | `ClusterShardingSettings.scala:243` | 未対応 | core/config + grain | medium | passivation 自体はあるが、strategy 設定階層（active entity limit、segmented LRU、admission window / filter）がない |
| `ShardingMessageExtractor` 実装群（`HashCodeMessageExtractor` / `HashCodeNoEnvelopeMessageExtractor` / `Murmur2MessageExtractor`） | `ShardingMessageExtractor.scala:52-101`, `Murmur2MessageExtractor.scala:19` | 未対応 | core/grain | easy | extractor SPI（上記行）成立後の標準実装群。Kafka 互換 Murmur2 partitioning 含む |
| remembered entities（`RememberEntitiesStore` / `StateStoreMode` / `RememberEntitiesStoreMode`） | `RememberEntitiesStore.scala:57`, `ClusterShardingSettings.scala:125` | 未対応 | core/placement + persistence integration | hard | activation registry はあるが、再起動 / rebalance 後にエンティティを再活性化する store 契約がない |
| external shard allocation（extension / strategy / client / `ShardLocations`） | `ExternalShardAllocation.scala:32`, `ExternalShardAllocationStrategy.scala:44` | 未対応 | core/placement + std | medium | 外部から shard 配置を指定する API がない |
| `ShardedDaemonProcess` / `ShardedDaemonProcessSettings` | `ShardedDaemonProcess.scala:30`, `ShardedDaemonProcessSettings.scala:27` | 未対応 | core/typed + placement | hard | N 個の daemon を shard 配置し keep-alive する API がない |
| replicated sharding（`ReplicatedShardingExtension` / `ReplicatedSharding` / `ReplicatedEntityProvider` / `ReplicatedEntity`） | `ReplicatedShardingExtension.scala:31`, `ReplicatedEntityProvider.scala:32` | 未対応 | core/typed + placement | hard | data center / replica id model がない |
| sharding delivery controllers（`ShardingProducerController` / `ShardingConsumerController`） | `ShardingProducerController.scala:104`, `ShardingConsumerController.scala:50` | 未対応 | core/typed + actor-core/delivery | hard | reliable delivery と sharding の接続がない |

実装済みとして扱うもの: `GrainRef`、`GrainKey`、`GrainCodec`、`VirtualActorRegistry`、`PlacementCoordinatorCore`、`PartitionIdentityLookup`、`RendezvousHasher`、`PidCache`、remote/local placement decision、passivation（基本機構）、RPC router（`GrainRpcRouter`）。

### 9. Distributed Data / CRDT　✅ 実装済み 0/27 (0%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `DistributedData` extension (classic) | `DistributedData.scala:27` | 未対応 | core + std | hard | replicator extension がない |
| typed `DistributedData` extension | `cluster-typed/ddata/typed/scaladsl/DistributedData.scala:33` | 未対応 | core/typed | medium | typed extension wrapper がない |
| `ReplicatorMessageAdapter[A, B]` | `cluster-typed/ddata/typed/scaladsl/ReplicatorMessageAdapter.scala:27` | 未対応 | core/typed | medium | typed Behavior と replicator protocol の連携 adapter がない |
| `Replicator` / `ReplicatorSettings` | `Replicator.scala:73`, `Replicator.scala:162` | 未対応 | core + std | hard | gossip-based CRDT replication 基盤（write/read repair、delta propagation、pruning 実行体）がない |
| Get protocol（`Get` / `GetSuccess` / `NotFound` / `GetFailure` / `GetDataDeleted`） | `Replicator.scala:428-488` | 未対応 | core/ddata | medium | 読み取りプロトコル型がない |
| Update protocol（`Update` / `UpdateSuccess` / `UpdateTimeout` / `ModifyFailure` / `StoreFailure` 等） | `Replicator.scala:591-679` | 未対応 | core/ddata | medium | 更新プロトコル型がない |
| Subscribe protocol（`Subscribe` / `Unsubscribe` / `Changed` / `Deleted`） | `Replicator.scala:504-547` | 未対応 | core/ddata | medium | 変更購読プロトコル型がない |
| Delete protocol（`Delete` / `DeleteSuccess` / `ReplicationDeleteFailure` / `DataDeleted`） | `Replicator.scala:695-715` | 未対応 | core/ddata | medium | 削除プロトコル型がない |
| read/write consistency levels（`ReadLocal` 〜 `ReadAll` / `WriteLocal` 〜 `WriteAll`、Majority/MajorityPlus 含む） | `Replicator.scala:298-379` | 未対応 | core/ddata | easy | 一貫性レベル契約がない |
| `GetReplicaCount` / `ReplicaCount` / `FlushChanges` | `Replicator.scala:726-742` | 未対応 | core/ddata | trivial | 補助 query / command 型 |
| `DurableStore` SPI（`Store` / `LoadAll` / `LoadData` protocol） | `DurableStore.scala:64-86` | 未対応 | core/ddata | medium | durable storage の port 契約がない |
| durable store std adapter（`LmdbDurableStore` 相当） | `DurableStore.scala:112` | 未対応 | std | medium | LMDB 完全互換ではなく、embedded KV による std 実装が対象 |
| `ReplicatedData` / `DeltaReplicatedData` / `ReplicatedDelta` / `RequiresCausalDeliveryOfDeltas` / `RemovedNodePruning` SPI | `ReplicatedData.scala:44-223` | 未対応 | core/ddata | medium | CRDT merge / delta / pruning の基底 trait 契約がない |
| `Key[T]` | `Key.scala:16` | 未対応 | core/ddata | easy | CRDT key hierarchy がない |
| `GCounter` / `PNCounter` / `PNCounterMap` | `GCounter.scala:22`, `PNCounter.scala:23`, `PNCounterMap.scala:24` | 未対応 | core/ddata | easy | 基本 counter CRDT がない |
| `Flag` | `Flag.scala:16` | 未対応 | core/ddata | trivial | enable-only CRDT |
| `LWWRegister` / `LWWMap` | `LWWRegister.scala:21`, `LWWMap.scala:21` | 未対応 | core/ddata | medium | timestamp / node ordering と clock 抽象が必要 |
| `ORSet` / `ORMap` / `ORMultiMap` | `ORSet.scala:43`, `ORMap.scala:24`, `ORMultiMap.scala:21` | 未対応 | core/ddata | medium | dot / tombstone / delta semantics が必要 |
| `VersionVector` | `VersionVector.scala:28` | 未対応 | core/ddata | medium | membership 用 `VectorClock` はあるが CRDT pruning / dot 管理用ではない |
| `SelfUniqueAddress` | `distributed-data/DistributedData.scala:80` | 未対応 | core/ddata | easy | CRDT 更新時の自ノード識別 newtype がない |

### 10. std adapter / discovery / wire integration　✅ 実装済み 9/11 (82%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `JoinConfigCompatCheckSharding` | `cluster-sharding/JoinConfigCompatCheckSharding.scala` | 未対応 | core/config | easy | `JoinCompatibilityComposition` は pubsub / downing / SBR / failure-detector key を合成するが、sharding 設定の join compat key がない（sharding settings 契約成立が前提） |
| module setup integration（`ClusterShardingSetup` / `ClusterSingletonSetup` 相当） | `cluster-sharding-typed/scaladsl/ClusterSharding.scala:541`, `cluster-typed/ClusterSingleton.scala:326` | 未対応 | core/typed + std | easy | sharding / singleton extension 自体が未実装のため従属的に未対応 |

実装済みとして扱うもの: cluster message serializer contract（`ClusterMessagePayloadKind` / `ClusterMessageManifest` / `ClusterSerializedMessage` / `ActorSerializationBridge` + std `ClusterWireFrameV1` / `ClusterWireCodec` / `ClusterWireDecodeFailure`）、seed node discovery process（`SeedNodeProcess` + `ProviderLifecycleBridge`）、generic discovery adapter（`DiscoveryBackend` / `GenericDiscoveryAdapter` / AWS ECS feature）、`ClusterApi` 公開面 parity（join / leave / subscribe / unsubscribe / current_state / down / get / request / remote_path_of のフルセット — 判定変更）、transport lifecycle bridge retention（`subscribe_remoting_events`）、gossip transport adapter（`TokioGossipTransport` / `TokioGossiper`）、provider lifecycle（`LocalClusterProvider` / `StaticClusterProvider` / `AwsEcsClusterProvider`）、versioned wire frame、discovery topology mapper。

## 対象外 (n/a)

| Pekko API / 領域 | 判定理由 |
|------------------|----------|
| `cluster-metrics` | デフォルト固定スコープ外。ユーザーが metrics 調査を明示した場合だけ対象 |
| `ClusterClient` / `ClusterClientReceptionist` / `ClusterClientSettings` / `ClusterReceptionistSettings` | Pekko 本体で全面 `@deprecated`（gRPC 移行推奨） |
| typed `ClusterReceptionist` 実装 | `@InternalApi`。receptionist 公開契約は actor-typed スコープ |
| SBR `DowningStrategy` 実装クラス / `SplitBrainResolverSettings`（Pekko 側） | 全て `@InternalApi private[sbr]`。公開契約（provider SPI / 設定契約 / 5 戦略）はカテゴリ3 で対象 |
| `ClusterJmx` / MBean | JVM management / JMX 固有 |
| HOCON loader / dynamic access | JVM 設定ロード方式に依存 |
| Java DSL wrapper / javadsl package | Rust API として再現不要 |
| multi-node-testkit / tests / typed tests / `TestEntityRef` | runtime API ではない |
| Kubernetes discovery backend 完全互換 | backend 固有実装。generic provider adapter だけ cluster scope |
| JFR flight recorder event classes | JVM Flight Recorder 固有 |
| Akka 互換 migration adapter / `RemoveInternalClusterShardingData` | 移行用実装であり runtime parity 対象外 |
| protobuf serializer の完全バイナリ互換 | Rust runtime contract では serializer 接続点だけ対象 |

## スタブ / placeholder

`todo!()` / `unimplemented!()` / `panic!("not implemented")` は実装本体から検出されなかった（`#![deny(clippy::todo)]` / `#![deny(clippy::unimplemented)]` が全クレートで有効）。`ClusterProviderShared` の rustdoc 例に `todo!()` があり、`cluster_pub_sub_impl.rs:534` に初期化デフォルト値を説明するコメント上の "placeholder" があるが、いずれも実行スタブではない。

## 内部モジュール構造ギャップ

今回は API / 実動作ギャップが支配的なため、内部モジュール構造ギャップの詳細分析は省略する。固定スコープ概念カバレッジは 58% で、判定基準（カバレッジ 80% 以上、または hard/medium 未実装 5 件以下）を満たさない。

次版で構造分析へ進む場合の観点:

| 構造観点 | 現状 | 次に見るべき点 |
|----------|------|----------------|
| membership と provider の境界 | pure coordinator と provider/event-stream adapter が分かれている | SBR runtime loop / CoordinatedShutdownLeave がどちらに入るべきか |
| gossip と wire の境界 | core gossip / heartbeat contract + std logical handoff + postcard delta UDP + actor-core serialization bridge | Pekko/protobuf 完全バイナリ互換を将来採用するか |
| grain と typed sharding の境界 | protoactor-go style の Grain API が中心 | Pekko typed sharding wrapper（Entity / EntityRef / Extractor）を薄く載せられるか |
| pubsub と distributed-data の境界 | PubSub は独自 broker、CRDT は未実装 | PubSub registry gossip を ddata Replicator 相当に寄せるか |
| singleton の配置 | 対応モジュールなし | cluster-tools 相当を core contract と std actor runtime にどう分けるか |

## 実装優先度

この節は「今の要求で実装すべきか」ではなく、「Pekko parity ギャップをどの順で埋めるか」を示す。YAGNI は適用せず、カテゴリ別ギャップに列挙済みの全項目を Phase 1〜3 に再配置する。着手時期の判断は roadmap / OpenSpec 側が行う。

### Phase 1: trivial / easy（既存設計の範囲で API surface を埋められるもの）

2026-06-11: cluster-membership-event-surface スペックで `Member.ordering` 公開契約、`ClusterLogMarker`（構造化 tracing field 契約）、`MemberPreparingForShutdown` / `MemberReadyForShutdown` event variant、`DataCenterReachabilityEvent` の 4 項目が実装完了し、本リストから除去した。

| 項目 | 実装先層 | 根拠カテゴリ |
|------|----------|--------------|
| `CrossDcFailureDetectorSettings` / Multi-DC 設定 namespace | core/config | カテゴリ1 |
| membership-driven routee add/remove の std 配線 | std | カテゴリ4 |
| `ClusterSingletonManagerSettings`（classic） | core/config | カテゴリ6 |
| `ClusterSingletonProxySettings` | core/config | カテゴリ6 |
| typed `ClusterSingletonSettings` | core/config | カテゴリ6 |
| typed `ClusterSingletonManagerSettings` | core/config | カテゴリ6 |
| `ClusterSingletonSetup` | core/typed + std | カテゴリ6 |
| `ClusterSingletonManagerIsStuck` 検知契約 | core | カテゴリ6 |
| mediator 全体 `Count` query | core/pub_sub | カテゴリ7 |
| `EntityTypeKey[M]` / typed `EntityRef[M]` wrapper | core/typed + grain | カテゴリ8 |
| `ClusterShardingHealthCheck` | std | カテゴリ8 |
| `ShardingMessageExtractor` 実装群（HashCode / Murmur2） | core/grain | カテゴリ8 |
| read/write consistency levels | core/ddata | カテゴリ9 |
| `Key[T]` | core/ddata | カテゴリ9 |
| `GCounter` / `PNCounter` / `PNCounterMap` | core/ddata | カテゴリ9 |
| `Flag` | core/ddata | カテゴリ9 |
| `GetReplicaCount` / `ReplicaCount` / `FlushChanges` | core/ddata | カテゴリ9 |
| `SelfUniqueAddress` | core/ddata | カテゴリ9 |
| `JoinConfigCompatCheckSharding` | core/config | カテゴリ10 |
| module setup integration（sharding / singleton setup 相当） | core/typed + std | カテゴリ10 |

注: singleton 設定群・setup・sharding extractor 実装群・ddata 型群は、それぞれ Phase 3 の基盤（singleton manager、extractor SPI、Replicator）が成立して初めて end-to-end で機能する。Phase 1 への分類は難易度基準によるものであり、着手順は依存先の Phase 3 項目と束ねて計画すること。

### Phase 2: medium（既存の core / std 境界の中で閉じるもの）

| 項目 | 実装先層 | 根拠カテゴリ |
|------|----------|--------------|
| `prepareForFullClusterShutdown` command path（kernel） | core + std | カテゴリ1 |
| `CoordinatedShutdownLeave` | core/extension + std | カテゴリ1 |
| `ClusterScope` deploy scope | core/extension | カテゴリ1 |
| `PrepareForFullClusterShutdown` command（typed） | core/typed + std | カテゴリ5 |
| `ClusterSingletonProxy` | std + core | カテゴリ6 |
| `SingletonActor[M]` | core/typed | カテゴリ6 |
| classic `ClusterSharding.start/startProxy` API | core/grain + std | カテゴリ8 |
| typed `ClusterSharding` extension | core/typed | カテゴリ8 |
| `Entity[M, E]` / `EntityContext` | core/typed + grain | カテゴリ8 |
| `ShardingEnvelope` / `ShardingMessageExtractor` SPI | core/grain | カテゴリ8 |
| `ClusterShardingSettings` 契約（classic + typed） | core/config | カテゴリ8 |
| sharding query protocol（classic + typed） | core/grain + core/typed | カテゴリ8 |
| passivation strategy settings（LRU / MRU / LFU / admission） | core/config + grain | カテゴリ8 |
| external shard allocation | core/placement + std | カテゴリ8 |
| typed `DistributedData` extension | core/typed | カテゴリ9 |
| `ReplicatorMessageAdapter[A, B]` | core/typed | カテゴリ9 |
| Get protocol | core/ddata | カテゴリ9 |
| Update protocol | core/ddata | カテゴリ9 |
| Subscribe protocol | core/ddata | カテゴリ9 |
| Delete protocol | core/ddata | カテゴリ9 |
| `DurableStore` SPI | core/ddata | カテゴリ9 |
| durable store std adapter | std | カテゴリ9 |
| `ReplicatedData` / delta / pruning 基底 SPI | core/ddata | カテゴリ9 |
| `LWWRegister` / `LWWMap` | core/ddata | カテゴリ9 |
| `ORSet` / `ORMap` / `ORMultiMap` | core/ddata | カテゴリ9 |
| `VersionVector` | core/ddata | カテゴリ9 |

### Phase 3: hard（新しい基盤・アーキテクチャ変更を要するもの）

| 項目 | 実装先層 | 根拠カテゴリ |
|------|----------|--------------|
| SBR runtime down execution loop（reachability 監視 → 責任ノード選択 → 自動 down 発行） | core + std | カテゴリ3 |
| concrete lease coordination backend | std | カテゴリ3 |
| classic `ClusterSingletonManager`（oldest election / handover） | std + core | カテゴリ6 |
| typed `ClusterSingleton` extension | core/typed + std | カテゴリ6 |
| shard allocation / rebalance strategy（SPI + least-shard + coordinator protocol） | core/placement | カテゴリ8 |
| remembered entities（store 契約 + StateStoreMode） | core/placement + persistence integration | カテゴリ8 |
| `ShardedDaemonProcess` / `ShardedDaemonProcessSettings` | core/typed + placement | カテゴリ8 |
| replicated sharding | core/typed + placement | カテゴリ8 |
| sharding delivery controllers | core/typed + actor-core/delivery | カテゴリ8 |
| `DistributedData` extension（classic） | core + std | カテゴリ9 |
| `Replicator` / `ReplicatorSettings`（CRDT replication 基盤） | core + std | カテゴリ9 |

### 対象外（n/a）

ClusterClient 系（deprecated）、`@InternalApi` 型、JMX / HOCON / JFR / Java DSL、testkit、protobuf バイナリ互換、`RemoveInternalClusterShardingData`。詳細は「対象外 (n/a)」表を正とする。

## まとめ

cluster は membership / gossip / heartbeat / reachability / downing decision model / typed Cluster facade / Grain runtime / PubSub / discovery provider / message serializer contract という基礎契約は強く、カテゴリ 1, 2, 4, 5, 7, 10 は 73〜93% のカバレッジに達している。全体カバレッジは 87/151 (58%) で、未実装の大半は Cluster Singleton（0%）、Distributed Data / CRDT（0%）、Pekko sharding public API 形態（41%）の 3 領域に集中している。

低コストで parity を前進できるのは、Phase 1 の `Member.ordering` 公開契約、shutdown / DC reachability イベント型、routee 更新の std 配線、mediator 全体 `Count`、基本 CRDT 型群（`Flag` / `GCounter` / `PNCounter` / consistency levels）、および Phase 2 の `prepareForFullClusterShutdown` command path（kernel + typed を 1 change で閉じられる）である。

parity 上の主要ギャップは Phase 3 に集約される: SBR runtime down execution loop（decision model は完成済みで、実行ループだけが欠けている）、Cluster Singleton（manager / proxy / typed extension）、sharding の rebalance / remembered entities / delivery controllers、そして Distributed Data の Replicator 基盤である。特に SBR runtime loop は、既存の `SplitBrainResolver` / `ReachabilityMatrix` / `MembershipCoordinator` が揃っているため、Phase 3 の中では最も着手可能性が高い。

API ギャップが支配的であり、内部モジュール構造の比較は、singleton / ddata / sharding public API の scope を採用する OpenSpec change が立った後に進めるのが妥当である。
