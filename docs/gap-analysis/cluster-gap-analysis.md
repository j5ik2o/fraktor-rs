# cluster モジュール ギャップ分析

更新日: 2026-06-05 (cluster-message-serialization-contract evidence reflected)

## 位置づけ

この文書は、`cluster-*` を Apache Pekko Cluster / Cluster Sharding 互換ロードマップとして扱うためのものではない。fraktor-rs の `cluster-*` は、Proto.Actor-Go 型の Virtual Actor / Grain runtime を主軸にする。Pekko は parity target ではなく、大規模運用で必要になる membership、reachability、downing、placement、rebalance などの失敗ケースと設計論点を確認する参照実装として扱う。

実装ロードマップは [2026-05-25_cluster-grain-runtime-roadmap.md](../plan/2026-05-25_cluster-grain-runtime-roadmap.md) を正とする。この gap analysis に列挙された typed Cluster API の未実装部分、Cluster Singleton、Cluster Client、Distributed Data、Pekko Sharding public API などは、未実装であること自体を直近の実装優先度とはみなさない。

現在の実装優先度は、Grain identity lookup、placement resolution、activation / passivation、membership topology update、cluster provider boundary、failure observation、downing decision contract を固めることにある。

詳細な Pekko gap table は、これらの運用 contract を設計するときの比較材料であり、raw API parity の backlog ではない。

### Deferred Pekko concepts

以下の Pekko 概念は、将来個別の OpenSpec change が採用するまで deferred とする。詳細表に出てくる場合も、現時点では比較 evidence として読む。

- typed Cluster API wrapper のうち未実装部分
- Cluster Singleton / ShardCoordinator parity
- Cluster Client / Receptionist
- Distributed Data / CRDT Replicator
- sharding delivery controllers
- replicated sharding / direct replication
- broad Pekko public API compatibility
- Pekko serializer binary compatibility

## 比較スコープ定義

この調査は、Apache Pekko cluster 配下の raw API 数をそのまま移植対象にするものではない。fraktor-rs の `cluster` では、cluster membership と virtual actor / sharding 相当の分散配置契約を対象にし、JVM 実装技術、Java/Scala DSL convenience、testkit、`cluster-metrics` は parity 分母から除外する。

### 対象に含めるもの

| 領域 | fraktor-rs | Pekko 参照 |
|------|------------|------------|
| cluster core | `modules/cluster-core-kernel/src/` (`activation`, `membership`, `extension`, `pub_sub`, `topology`) | `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/` |
| typed cluster contract | `modules/cluster-core-typed/src/` | `references/pekko/cluster-typed/src/main/scala/` |
| sharding / virtual actor | `modules/cluster-core-kernel/src/grain/`, `modules/cluster-core-kernel/src/activation/` | `references/pekko/cluster-sharding/`, `references/pekko/cluster-sharding-typed/` |
| cluster tools | `modules/cluster-core-kernel/src/pub_sub/` | `references/pekko/cluster-tools/src/main/scala/org/apache/pekko/cluster/pubsub/`, `singleton/`, `client/` |
| distributed data | 対応モジュールなし | `references/pekko/distributed-data/src/main/scala/org/apache/pekko/cluster/ddata/` |
| std adapter | `modules/cluster-adaptor-std/src/` | gossip transport / provider / discovery adapter として Rust で再現可能な契約 |

### 対象から除外するもの

| 除外項目 | 理由 |
|----------|------|
| `cluster-metrics` | スキル定義で明示的に別スコープ。fraktor 側に簡易 metrics はあるが parity 分母には入れない |
| Kubernetes / discovery backend 固有実装の完全互換 | backend 実装技術ごとの互換は std adapter の別調査対象 |
| multi-node-testkit / cluster tests / typed tests | runtime API ではない |
| Java DSL / Scala DSL convenience / implicit syntax | Rust API として再現する対象ではない |
| JVM management / JMX / HOCON dynamic loading / classloader | JVM 固有 |
| protobuf serializer の完全バイナリ互換 | contract 接続は対象だが、JVM serializer 実装そのものは対象外 |
| JFR / log marker の JVM 固有 event class | Rust 側は tracing / event stream contract として扱う |

### raw 抽出値の扱い

固定スコープ対象ディレクトリを `src/main` ベースで raw 抽出すると、Pekko 側は型宣言 857 件、主要 `def` 3027 件が見つかる。Pekko submodule は `2dc8960074bfe269da1686609eb88663cb50ad8b` を参照した。これには private / internal / JVM 固有 / DSL wrapper / serializer 実装が含まれるため、parity カバレッジ分母には使わない。

fraktor-rs 側はスキル指定の `pub` 系抽出で、型 294 件 (core-kernel: 263, core-typed: 9, std: 22)、公開メソッド 825 件 (core-kernel: 714, core-typed: 32, std: 79)。ただし、この数には `pub(crate)` の helper も含まれる。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 約 121 |
| fraktor-rs 固定スコープ対応概念 | 約 72 |
| 固定スコープ概念カバレッジ | 約 72/121 (60%) |
| raw public type declarations | 294 (core-kernel: 263, core-typed: 9, std: 22) |
| raw public method declarations | 825 (core-kernel: 714, core-typed: 32, std: 79) |
| hard gap | 16 |
| medium gap | 16 |
| easy gap | 9 |
| trivial gap | 2 |
| panic 系スタブ | 0 件 |
| 機能 placeholder / TODO | 0 件 |

注: ここでの `raw public` は `pub(crate)` など内部到達可能な `pub` を含む参考値であり、crate 外から到達可能な外部公開 API 数ではない。

cluster は、membership table、gossip dissemination、full gossip state merge / tombstone / seen digest、dedicated heartbeat evidence、failure detector registry、downing/SBR decision model、typed cluster facade、Grain/Placement/Identity、PubSub broker / topic registry gossip、UDP gossip transport などの基礎部品はかなり揃っている。一方で Pekko comparison の範囲で見ると、SBR runtime actor / down execution loop、cluster singleton/client、Pekko sharding の public API、Distributed Data/CRDT が大きく未実装である。

旧版は raw Scala 宣言数をサマリーに置きつつ、`cluster-metrics` を混ぜ、`ShardedDaemonProcess` や typed API を YAGNI で n/a にしていた。固定スコープ版では、JVM 固有以外の public runtime contract を comparison gap として保持する。ただし、直近の implementation backlog は cluster Grain runtime roadmap 側で管理する。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core / membership | `Cluster`, `Member`, `MemberStatus`, `CurrentClusterState`, `ClusterEvent`, `Gossip`, `Reachability` | `ClusterExtension`, `ClusterApi`, `NodeRecord`, `NodeStatus`, `CurrentClusterState`, `MembershipCoordinator`, `GossipDisseminationCoordinator`, `ReachabilityMatrix`, `GossipStateModel`, `HeartbeatProtocolState`, `CrossDcHeartbeat` | UniqueAddress、data center、WeaklyUp、reachability matrix、gossip envelope、full gossip merge / tombstone / seen digest、dedicated heartbeat evidence、SeedNodeProcess の core contract はある |
| core / downing | `DowningProvider`, `NoDowning`, SBR | `DowningProvider`, `DowningInput`, `DowningDecisionContext`, `DowningStrategyDecision`, `DowningDecisionTrace`, `FailureObservation`, `IndirectConnectionEvidence`, `NoopDowningProvider`, `SplitBrainResolver`, `LeaseMajorityPort`, `SplitBrainResolverProviderHook` | core decision model は SBR strategy と lease majority outcome を評価できる。SBR runtime actor / reachability change subscription / down execution loop は未実装 |
| core / typed | typed `Cluster`, command, subscription, singleton, sharding typed API | `modules/cluster-core-typed` に typed `Cluster` / command / state subscription / self events / setup がある | typed Cluster の薄い facade はあるが、singleton / sharding typed API は未実装 |
| core / virtual actor | `ClusterSharding`, `EntityRef`, `EntityTypeKey`, `ShardRegion`, coordinator | `GrainRef`, `GrainKey`, `VirtualActorRegistry`, `PlacementCoordinatorCore`, `PartitionIdentityLookup` | protoactor-go style の同等機能は強いが Pekko public API と remember/rebalance が不足 |
| core / distributed state | `DistributedData`, `Replicator`, CRDT 型群 | なし | 未実装 |
| std / adapter | gossip transport, provider, discovery adapter | `TokioGossipTransport`, `MembershipCoordinatorDriver`, `LocalClusterProvider`, `StaticClusterProvider`, `AwsEcsClusterProvider`, `GenericDiscoveryAdapter`, `ProviderLifecycleBridge`, `ClusterWireCodec` | Rust adapter、logical envelope handoff、seed/discovery provider boundary、cluster message serializer contract はある。Pekko/protobuf 完全バイナリ互換は scope 外 |

## カテゴリ別ギャップ

### 1. Cluster membership / lifecycle　✅ 実装済み 16/17 (94%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `UniqueAddress` semantics | `Member.scala:315`, `Member.scala:331` | core contract 実装済み | core/membership | medium | `NodeRecord::unique_address` と `MembershipTable::try_join_with_identity` が address + UID を member identity として保持する。同一 address + 別 UID は別 incarnation、UID 未確定は `UnconfirmedIdentity`。tests: `node_record_with_identity_*`, `join_with_identity_*` |
| data center membership | `Cluster.scala:102`, `ClusterEvent.scala:396` | core contract 実装済み | core/membership | medium | `DataCenter` と `NodeRecord::data_center`、`MembershipSnapshot::members_in_data_center`、`CurrentClusterState::{members,unreachable}_in_data_center`。Cross-DC heartbeat evidence は別行で実装済み。routing / discovery / downing policy は downstream scope |
| `WeaklyUp` / full member status compatibility | `Member.scala:241`, `ClusterEvent.scala:279` | core contract 実装済み | core/membership | easy | `NodeStatus::WeaklyUp`、`Joining -> WeaklyUp -> Up`、WeaklyUp から leave/remove/down への transition、`is_provisional` helper。tests: `join_then_heartbeats_promote_through_weakly_up_to_up`, `weakly_up_*` |
| `prepareForFullClusterShutdown` | `Cluster.scala:336`, `cluster-typed/Cluster.scala:175` | 部分実装 | core + std | medium | `PreparingForShutdown` / `ReadyForShutdown` は型だけあり、full shutdown command path がない |
| `remotePathOf` | `Cluster.scala:442` | 実装済み | core/extension | easy | `ClusterApi::remote_path_of` が local ref を advertised authority 付き canonical remote path にし、既存 remote authority と UID を保持する。`remote_path_of_*` tests で確認 |

実装済みとして扱うもの: cluster extension、join/leave/down、event stream subscription、current state snapshot、member/up/removed callback、roles/app_version 設定、leader/role leader 算出、startup/shutdown event。

### 2. Gossip / reachability / failure detection　✅ 実装済み 14/15 (93%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `Reachability` matrix | `Reachability.scala:36`, `Reachability.scala:38` | core contract 実装済み | core/membership | medium | `ReachabilityMatrix` が observer / subject / status / version を保持し、reachable prune、terminated precedence、aggregate status、snapshot propagation を提供する。MembershipCoordinator は local observer の suspect/reachable を matrix に反映する。downing decision は所有しない |
| full `Gossip` merge / tombstone / seen digest | `Gossip.scala:127`, `Gossip.scala:178`, `Gossip.scala:230` | core contract 実装済み | core/membership | hard | `GossipStateModel` / `GossipStateSnapshot` が full state merge、deterministic precedence、removed/dead tombstone、seen digest convergence、tombstone retention prune を扱う。tests: `full_state_merge_*`, `tombstone_*`, `seen_digest_tracks_peer_observed_versions_and_convergence` |
| `GossipEnvelope` | `Gossip.scala:307` | core + logical handoff 実装済み | core/membership + std/handoff | medium | `GossipEnvelope` が from/to `UniqueAddress`、payload kind、membership version、deadline を保持する。`GossipTransportHandoff` / `TokioGossipTransport` は identity と payload kind を失わない logical handoff を扱う。serializer bytes は downstream scope |
| dedicated `ClusterHeartbeatSender` / receiver protocol | `ClusterHeartbeat.scala:82`, `ClusterHeartbeat.scala:90` | core protocol 実装済み | core/membership | medium | `HeartbeatProtocolState` が peer ごとの sequence、request/response 照合、first / regular timeout evidence を扱う。evidence は reachability input であり downing decision は実行しない。tests: `heartbeat_*` |
| `CrossDcClusterHeartbeat` | `CrossDcClusterHeartbeat.scala:230` | core evidence 実装済み | core/membership | hard | `CrossDcHeartbeat` が data center pair、cross-DC request/response、target add/remove/retain、timeout evidence を扱う。routing、discovery、downing strategy は決定しない。tests: `cross_dc_*` |
| `SeedNodeProcess` | `SeedNodeProcess.scala:22` | core + std provider boundary 実装済み | core/cluster_provider + std/provider | medium | `SeedNodeInput` と `SeedNodeProcess` が empty seed、self filtering、duplicate seed、invalid authority、client start、shutdown 後停止を扱う。`ProviderLifecycleBridge` が seed input を topology update に接続する。tests: `seed_node_process_*`, `provider_lifecycle_bridge_seed_input_publishes_topology_update`, `provider_lifecycle_bridge_shutdown_stops_seed_and_discovery_lifecycle` |
| config compatibility full key set | `JoinConfigCompatChecker.scala:25`, `JoinConfigCompatCheckCluster.scala:27` | baseline 実装済み | core/config | easy | `ClusterCompatibilityKeyCatalog` が required/excluded key を公開し、`JoinCompatibilityComposition` と `ClusterExtensionConfig::check_join_compatibility` が pubsub、downing provider、SBR settings の mismatch reason を合成する。`cluster_compatibility_key` / `join_compatibility` tests で確認 |
| failure detector implementation choice | `Cluster.scala:124`, `Cluster.scala:131` | 部分実装 | core/failure_detector | easy | registry はあるが cluster config から deadline/phi などを選ぶ設定 contract がない |

実装済みとして扱うもの: `MembershipTable`、`MembershipDelta`、`MembershipVersion`、`VectorClock`、`DefaultFailureDetectorRegistry`、`MembershipCoordinator::poll` による suspect/dead 遷移、`GossipEnvelope`、`GossipStateModel`、`HeartbeatProtocolState`、`CrossDcHeartbeat`、logical envelope handoff、`TokioGossipTransport`。

### 3. Downing / Split Brain Resolver　✅ decision model 実装済み / runtime actor remaining

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `SplitBrainResolver` | `SplitBrainResolver.scala:50`, `SplitBrainResolver.scala:160` | core evaluator 実装済み | core/downing_provider + std/provider | hard | `SplitBrainResolver` が `DowningDecisionContext` を評価し、KeepMajority / fixed-size StaticQuorum / KeepOldest / DownAll / LeaseMajority の trace 付き decision を返す。stable-after は pending trace、down-all timeout は all-down trace として観測できる。SBR runtime actor、責任ノード選択、reachability 変化監視、provider からの実 down loop は未実装 |
| `DowningStrategy` / decision model | `DowningStrategy.scala:34`, `DowningStrategy.scala:70` | core decision model 実装済み | core/downing_provider | hard | `DowningStrategyDecision` と `DowningDecisionTrace` が keep/down/defer/all-down、retained partition、downing targets、tie-break、stable-after、down-all timeout、lease outcome を保持する。tests: `downing_strategy_decision`, `split_brain_resolver` |
| `SplitBrainResolverProvider` | `SplitBrainResolverProvider.scala` | provider-facing binding 実装済み | core/downing_provider + std/provider | easy | `SplitBrainResolverProviderHook` が provider key、SBR settings、strategy identity を compatibility metadata として公開し、trace decision を `DowningDecision` / `ClusterProviderError` に変換する。`StdSplitBrainResolverProvider` は lifecycle start 時に hook と lease backend adapter を構成し、stop/drop で backend state を close する。tests: `split_brain_resolver_provider_hook`, `split_brain_resolver_provider` |
| lease-based majority | `DowningStrategy.scala:602` | port contract + std binding 実装済み | core/downing_provider + std/provider | hard | `LeaseMajorityPort` / `LeaseAcquisitionOutcome` が acquired、denied、unavailable、unknown、backend missing を区別し、`SplitBrainResolver::decide_with_lease` が lease acquired の場合だけ retained partition を keep する。tie partition でも lease port を consult する。std 側は `StdLeaseMajorityBackend` を provider lifecycle 内で所有する。concrete coordination backend / retry / network I/O は未実装 |
| indirect connection handling | `DowningStrategy.scala:245` | evidence contract + decision input 実装済み | core/membership + core/downing_provider | medium | `ReachabilityMatrix::indirect_evidence_for` と `IndirectConnectionEvidence` が direct / indirect observation、observer aggregate status、direct-only fallback を表現する。`DowningDecisionContext` は `DowningInput::IndirectConnectionEvidence` を SBR evaluation input に変換できる |

実装済みとして扱うもの: `DowningProvider` trait、`DowningInput` / `DowningDecision` / `DowningDecisionContext` / `DowningStrategyDecision` / `DowningDecisionTrace`、`FailureObservation` / `IndirectConnectionEvidence`、`NoopDowningProvider`、`SplitBrainResolverSettings` / `SplitBrainResolverStrategy` / `SplitBrainResolver`、`LeaseMajorityPort` / `LeaseAcquisitionOutcome`、`SplitBrainResolverProviderHook`、`StdSplitBrainResolverProvider`、明示 `ClusterApi::down` hook、downing provider / SBR settings の join compatibility。

### 4. Cluster router pool / group　✅ 実装済み 6/6 (100%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| role-filtered routee selection | `ClusterRouterConfig.scala:80`, `ClusterRouterConfig.scala:190` | 実装済み | core/router | easy | `ClusterRouterPoolConfig` / `ClusterRouterGroupConfig` に `use_roles` + `satisfies_roles`（Pekko `useRoles.subsetOf`）|
| max instances per node | `ClusterRouterConfig.scala:190` | 実装済み | core/router | easy | pool config に `max_instances_per_node`。`ClusterRouterPool::from_candidates` が per-node / total cap を尊重した least-loaded 配置を行う |
| membership-driven routee add/remove | `ClusterRouterConfig.scala:586`, `ClusterRouterConfig.scala:591` | 実装済み (core policy) | core/router | medium | `ClusterRouterPool::update_from_members`（status==Up / role / allow-local フィルタ → 再配置）と group `local_routee_paths`。ClusterEvent 購読 loop の std 配線は別途のアダプタ作業 |

実装済みとして扱うもの: `ClusterRouterPool`、`ClusterRouterGroup`、pool/group settings の分離、`use_roles` / `satisfies_roles`、`max_instances_per_node`、`ClusterRouterPool::from_candidates` / `update_from_members`、group `local_routee_paths`。

### 5. Cluster Typed API　✅ 実装済み 6/7 (86%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `PrepareForFullClusterShutdown` command | `cluster-typed/Cluster.scala:175` | 未対応 | core/typed + std | medium | core lifecycle command と coordinated shutdown 接続が必要 |

実装済みとして扱うもの: typed `Cluster` facade、`ClusterCommand` の Join / JoinSeedNodes / Leave / Down、`ClusterStateSubscription`、typed event delivery、`SelfUp`、`SelfRemoved`、`ClusterSetup`。

### 6. Cluster singleton / client / receptionist　✅ 実装済み 0/12 (0%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| typed `ClusterSingleton` extension | `cluster-typed/ClusterSingleton.scala:135`, `cluster-typed/ClusterSingleton.scala:210` | 未対応 | core/typed + std | hard | cluster 全体で一つの actor を保証する coordinator がない |
| `SingletonActor[M]` | `cluster-typed/ClusterSingleton.scala:153`, `cluster-typed/ClusterSingleton.scala:171` | 未対応 | core/typed | medium | singleton entity 設定 wrapper がない |
| `ClusterSingletonSettings` | `cluster-typed/ClusterSingleton.scala:32`, `cluster-typed/ClusterSingleton.scala:57` | 未対応 | core/config | easy | role / removal margin / lease 等の設定がない |
| classic `ClusterSingletonManager` | `ClusterSingletonManager.scala:173`, `ClusterSingletonManager.scala:492` | 未対応 | std + core | hard | leader election、handover、termination message が必要 |
| `ClusterSingletonProxy` | `ClusterSingletonProxy.scala:135`, `ClusterSingletonProxy.scala:171` | 未対応 | std + core | medium | singleton location 追跡と proxy 送信がない |
| `ClusterClient` | `ClusterClient.scala:292`, `ClusterClient.scala:381` | 未対応 | std | hard | 外部 client、contact point、heartbeat、buffering がない |
| `ClusterClientReceptionist` | `ClusterClient.scala:565`, `ClusterClient.scala:583` | 未対応 | std + pub_sub | hard | service/subscriber registration と receptionist actor がない |
| `ClusterReceptionistSettings` | `ClusterClient.scala:661`, `ClusterClient.scala:713` | 未対応 | core/config | easy | receptionist 設定型がない |

### 7. Distributed PubSub　✅ 実装済み 10/10 (100%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `DistributedPubSubMediator` protocol | `DistributedPubSubMediator.scala:151`, `DistributedPubSubMediator.scala:553` | 実装済み | core/pub_sub + std | medium | `MediatorCommand` / `MediatorAcknowledgement` / `MediatorQueryResult` / `DistributedPubSubMediatorState` が subscribe / publish / unsubscribe / query を registry と delivery intent へ接続。std は `PubSubDeliveryIntentExecutor` で intent を実行 |
| `DistributedPubSubSettings` | `DistributedPubSubMediator.scala:44`, `DistributedPubSubMediator.scala:103` | 実装済み | core/pub_sub | easy | `DistributedPubSubSettings` が role / routing mode / gossip interval / removed TTL / max delta elements / no-subscriber behavior を保持 |
| topic registry gossip / delta collection | `DistributedPubSubMediator.scala:699`, `DistributedPubSubMediator.scala:861` | 実装済み | core/pub_sub + membership | hard | `TopicRegistryStatus` / `TopicRegistryDelta` / `TopicRegistryDeltaCollector` / `TopicRegistryGossipPayload` / `PubSubGossipHandoff` が owner version status、bounded delta、logical pubsub gossip kind を提供 |
| `Send` / `SendToAll` path semantics | `DistributedPubSubMediator.scala:206`, `DistributedPubSubMediator.scala:216` | 実装済み | core/pub_sub + actor-core | medium | `MediatorPathKey` と `PubSubPathSemantics` が address-less canonical path、one-of、local affinity、all-of、all-but-self、no-subscriber intent を提供 |

実装済みとして扱うもの: `ClusterPubSub` trait、`ClusterPubSubImpl`、`PubSubBroker`、topic / subscriber / publish ack、delivery policy、partition behavior、std `PubSubDeliveryActor`、mediator command protocol、distributed pub-sub settings、path semantics、topic registry status / delta / gossip handoff。

### 8. Sharding / Grain / Placement / Identity　✅ 実装済み 11/19 (58%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| classic `ClusterSharding.start/startProxy` API | `ClusterSharding.scala:224`, `ClusterSharding.scala:516` | 部分実装 | core/grain + std | medium | `setup_member_kinds` / `GrainRef` はあるが Pekko 風 start/startProxy API はない |
| typed `ClusterSharding` extension | `typed/scaladsl/ClusterSharding.scala:40`, `typed/scaladsl/ClusterSharding.scala:178` | 部分実装 | core/typed | medium | grain API はあるが typed `EntityRef` API ではない |
| `Entity[M, E]` / `EntityContext` | `typed/scaladsl/ClusterSharding.scala:238`, `typed/scaladsl/ClusterSharding.scala:363` | 部分実装 | core/typed + grain | medium | `ActivatedKind` / `GrainContext` は対応するが typed behavior factory ではない |
| `EntityTypeKey[M]` / typed `EntityRef[M]` | `typed/scaladsl/ClusterSharding.scala:407`, `typed/scaladsl/ClusterSharding.scala:439` | 部分実装 | core/typed + grain | easy | `GrainKey` / `GrainRef` はあるが typed key/ref wrapper がない |
| `ShardingEnvelope` / `ShardingMessageExtractor` | `ShardingMessageExtractor.scala:52`, `ShardingMessageExtractor.scala:124` | 部分実装 | core/grain | medium | `SerializedMessage` / `GrainCodec` はあるが envelope extractor 契約がない |
| shard allocation / rebalance strategy | `ClusterSharding.scala:669`, `ShardCoordinator.scala:662` | 部分実装 | core/placement | hard | rendezvous hashing はあるが least-shard rebalance と coordinator protocol はない |
| remembered entities | `Shard.scala:66`, `RememberEntitiesStore.scala:57` | 未対応 | core/placement + persistence integration | hard | activation registry はあるが remembered entity store がない |
| external shard allocation | `ExternalShardAllocation.scala:32`, `ExternalShardAllocationStrategy.scala:44` | 未対応 | core/placement + std | medium | external allocation API がない |
| `ShardedDaemonProcess` | `ShardedDaemonProcess.scala:30`, `ShardedDaemonProcess.scala:49` | 未対応 | core/typed + placement | hard | N 個の daemon を shard 配置する API がない |
| replicated sharding / direct replication | `ReplicatedEntityProvider.scala:32`, `ShardingDirectReplication.scala` | 未対応 | core/typed + placement | hard | data center / replica id model がない |
| sharding delivery controllers | `ShardingProducerController.scala:104`, `ShardingConsumerController.scala:50` | 未対応 | core/typed + actor-core/delivery | hard | reliable delivery と sharding の接続がない |

実装済みとして扱うもの: `GrainRef`、`GrainKey`、`GrainCodec`、`VirtualActorRegistry`、`PlacementCoordinatorCore`、`PartitionIdentityLookup`、`RendezvousHasher`、`PidCache`、remote/local placement decision、passivation、RPC router。

### 9. Distributed Data / CRDT　✅ 実装済み 0/18 (0%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `DistributedData` extension | `DistributedData.scala:27`, `DistributedData.scala:42` | 未対応 | core + std | hard | replicator extension がない |
| `Replicator` / `ReplicatorSettings` | `Replicator.scala:73`, `Replicator.scala:284`, `Replicator.scala:1183` | 未対応 | core + std | hard | gossip-based CRDT replication 基盤がない |
| `ReplicatedData` trait family | `ReplicatedData.scala:44`, `ReplicatedData.scala:69`, `ReplicatedData.scala:112` | 未対応 | core/ddata | medium | CRDT merge/delta/pruning trait がない |
| `Key[T]` / typed keys | `Key.scala:16`, `Key.scala:37` | 未対応 | core/ddata | easy | CRDT key hierarchy がない |
| `VersionVector` | `VersionVector.scala:28`, `VersionVector.scala:337` | 未対応 | core/ddata | medium | membership 用 `VectorClock` はあるが CRDT pruning/vector version ではない |
| `GCounter` / `PNCounter` | `GCounter.scala:22`, `PNCounter.scala:23` | 未対応 | core/ddata | easy | 基本 counter CRDT がない |
| `Flag` | `Flag.scala:16`, `Flag.scala:50` | 未対応 | core/ddata | trivial | enable-only CRDT |
| `LWWRegister` / `LWWMap` | `LWWRegister.scala:21`, `LWWMap.scala:21` | 未対応 | core/ddata | medium | timestamp / node ordering が必要 |
| `ORSet` / `ORMap` / `ORMultiMap` | `ORSet.scala:43`, `ORMap.scala:24`, `ORMultiMap.scala:21` | 未対応 | core/ddata | medium | dot / tombstone / delta semantics が必要 |
| `PNCounterMap` | `PNCounterMap.scala:24` | 未対応 | core/ddata | easy | PNCounter + map 合成 |
| read/write consistency levels | `Replicator.scala:284` | 未対応 | core/ddata | easy | ReadLocal/ReadMajority/WriteMajority 等 |
| typed DistributedData API | `cluster-typed/ddata/typed/scaladsl/DistributedData.scala:33` | 未対応 | core/typed | medium | typed actor ref adapter が必要 |

### 10. std adapter / discovery / wire integration　✅ 実装済み 8/9 (89%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| cluster message serializer contract | `ClusterMessageSerializer.scala:83`, `ClusterShardingMessageSerializer.scala`, `DistributedPubSubMessageSerializer.scala` | contract 実装済み | std/wire + actor-core serialization | hard | core `message_serialization` は `ClusterMessagePayloadKind`、`ClusterMessageManifest`、`ClusterSerializedMessage`、`ActorSerializationBridge` を公開する。std `message_wire` は `ClusterWireFrameV1`、`ClusterWireCodec`、`ClusterWireDecodeFailure` を公開する。tests: actor-core metadata preservation、unknown payload/version/malformed payload failure、gossip/pubsub wire smoke。gossip semantics、pubsub mediator semantics、transport lifecycle、Pekko/protobuf 完全バイナリ互換は scope 外 |
| seed node discovery process | `SeedNodeProcess.scala:22` | boundary contract 実装済み | core/cluster_provider + std/provider | medium | `SeedNodeProcess` が seed authority を provider-neutral join input に変換し、`DiscoveryTopologyMapper` が topology update へ写像する。`ProviderLifecycleBridge` が lifecycle 内で seed/discovery input を同じ contract に接続する。tests: `seed_node_process_*`, `discovery_topology_mapper_*`, `provider_lifecycle_bridge_*` |
| generic discovery adapter | `Cluster.scala:354`, `ClusterClient.scala:65` | boundary contract 実装済み | core/cluster_provider + std/provider | medium | `DiscoveryBackend` / `DiscoveryBackendError` / `GenericDiscoveryAdapter` が backend success、empty success、failure、AWS ECS style authority を `DiscoveryResult` / `DiscoveredAuthority` に正規化する。tests: `generic_discovery_adapter_*`, `discovery_result_*`, `aws_ecs` feature tests |
| std `ClusterApi` wrapper parity | `Cluster.scala:328`, `Cluster.scala:384`, `Cluster.scala:395` | 部分実装 | std/api | trivial | std wrapper は `get/request/down` のみで `join/leave/subscribe` を再公開していない |
| transport lifecycle to membership bridge retention | `local_cluster_provider_ext.rs` | 実装済み | std/provider | easy | `subscribe_remoting_events` が `EventStreamSubscription` を返し、guard 保持中だけ connected/quarantined events を topology input にする。weak provider retention により subscription は provider を延命しない。`local_cluster_provider_ext` tests で確認 |

実装済みとして扱うもの: `TokioGossipTransport`、`MembershipCoordinatorDriver`、`LocalClusterProvider`、`StaticClusterProvider`、`AwsEcsClusterProvider`、`SeedNodeProcess`、`DiscoveryTopologyMapper`、`GenericDiscoveryAdapter`、`ProviderLifecycleBridge`、`ClusterMessagePayloadKind`、`ClusterMessageManifest`、`ClusterSerializedMessage`、`ActorSerializationBridge`、`ClusterWireFrameV1`、`ClusterWireCodec`、`ClusterWireDecodeFailure`。logical gossip envelope handoff は `GossipEnvelope` evidence の一部として扱い、この section の独立 completed concept には数えない。

## 対象外 (n/a)

| Pekko API / 領域 | 判定理由 |
|------------------|----------|
| `cluster-metrics` | デフォルト固定スコープ外。ユーザーが metrics 調査を明示した場合だけ対象 |
| `ClusterJmx` / MBean | JVM management / JMX 固有 |
| HOCON loader / dynamic access | JVM 設定ロード方式に依存 |
| Java DSL wrapper / javadsl package | Rust API として再現不要 |
| multi-node-testkit / tests / typed tests | runtime API ではない |
| Kubernetes discovery backend 完全互換 | backend 固有実装。generic provider adapter だけ cluster scope |
| JFR flight recorder event classes | JVM Flight Recorder 固有 |
| Akka 互換 migration adapter | Pekko の移行用実装であり runtime parity 対象外 |
| protobuf serializer の完全バイナリ互換 | Rust runtime contract では serializer 接続点だけ対象 |

## スタブ / placeholder

`todo!()` / `unimplemented!()` / `panic!("not implemented")` は実装本体から検出されなかった。`ClusterProviderShared` の rustdoc 例に `todo!()` があり、`cluster_extension_test.rs` などの test helper 名に `stub` が出るが、いずれも実行スタブではない。

## Pekko comparison gap の優先度メモ

この section は、Pekko 側の概念を将来採用する場合の難易度メモであり、現在の cluster roadmap ではない。直近の作業順は [cluster Grain runtime roadmap](../plan/2026-05-25_cluster-grain-runtime-roadmap.md) と個別の OpenSpec change を正とする。

以下では、現在の Grain runtime roadmap に隣接する比較材料と、OpenSpec change が立つまで優先度を落とす `Deferred Pekko concepts` を分ける。deferred 側は fixed-scope gap の evidence として保持するが、active comparison follow-up と同じ backlog には入れない。

### Active comparison follow-up: trivial / easy

| 項目 | 実装先層 | 状態 | 根拠 / evidence |
|------|----------|------|-----------------|
| config compatibility full key set | core/config | baseline 実装済み | `ClusterCompatibilityKeyCatalog`、`JoinCompatibilityComposition`、`ClusterExtensionConfig::check_join_compatibility`。tests: `cluster_compatibility_key`, `join_compatibility` |
| `remotePathOf` | core/extension | 実装済み | `ClusterApi::remote_path_of`。tests: `remote_path_of_*` |
| transport lifecycle bridge retention | std/provider | 実装済み | `subscribe_remoting_events`、`EventStreamSubscription`、weak provider retention。tests: `local_cluster_provider_ext` |

上記は cluster-active-compatibility-baseline の完了 evidence であり、次の項目を完了扱いにしない: seed process / discovery provider、pubsub mediator semantics / registry gossip、Deferred Pekko concepts。`SplitBrainResolverProvider` は下の `cluster-downing-sbr-decision-model` evidence に移動し、cluster message serializer contract は専用 completed table に分離した。

### Completed active follow-up: membership / reachability model

| 項目 | 実装先層 | 状態 | 根拠 / evidence |
|------|----------|------|-----------------|
| `UniqueAddress` semantics | core/membership | core contract 実装済み | `NodeRecord::unique_address`、`MembershipTable::try_join_with_identity`、same address + different UID の別 incarnation、UID 未確定 rejection。tests: `node_record_with_identity_keeps_unique_address_and_data_center`, `join_with_identity_*` |
| data center membership | core/membership | core contract 実装済み | `DataCenter`、`NodeRecord::data_center`、membership/current state data center filtering。tests: `data_center::*`, `members_in_data_center_preserves_identity_status_and_roles`, `current_cluster_state_filters_members_by_data_center_without_losing_status` |
| `WeaklyUp` compatibility | core/membership | core contract 実装済み | `NodeStatus::WeaklyUp`、`mark_weakly_up`、`Joining -> WeaklyUp -> Up`、leave/remove/down transition、`is_provisional`。tests: `join_then_heartbeats_promote_through_weakly_up_to_up`, `joining_member_transitions_through_weakly_up_before_up`, `weakly_up_*` |
| `Reachability` matrix | core/membership | core contract 実装済み | `ReachabilityMatrix` / `ReachabilityRecord` / `ReachabilitySnapshot` が observer / subject / status / version、reachable prune、terminated precedence、aggregate status、snapshot propagation を保持。tests: `reachability_matrix::*`, `reachability_snapshot_tracks_failure_detector_and_heartbeat_receipt` |
| indirect connection handling | core/membership + core/downing_provider | evidence contract 実装済み | `IndirectConnectionEvidence` と `DowningInput::IndirectConnectionEvidence` が partial connectivity evidence を渡す。downing decision、lease majority、SBR strategy は実行しない。tests: `indirect_connection_evidence::*` |

この completed table は `cluster-membership-reachability-model` の core contract evidence だけを表す。SeedNodeProcess / generic discovery adapter は discovery provider interop の completed table に分離し、downing / SBR decision model と PubSub mediator / topic registry gossip はそれぞれ専用の completed table に分離する。次の責務は未完了のまま downstream / future scope に残す: SBR runtime actor、provider からの実 down execution loop、concrete lease coordination backend、Deferred Pekko concepts。

### Completed active follow-up: gossip / heartbeat protocol

| 項目 | 実装先層 | 状態 | 根拠 / evidence |
|------|----------|------|-----------------|
| `GossipEnvelope` | core/membership + std/handoff | core + logical handoff 実装済み | `GossipEnvelope` と `GossipPayloadKind` が from/to identity、payload kind、membership version、deadline を保持する。`GossipTransportHandoff` と `TokioGossipTransport` が identity / endpoint mapping / payload kind を logical handoff として保持する。tests: `gossip_envelope_*`, `handoff_*`, `envelope_roundtrip_distinguishes_gossip_state_and_heartbeat_payloads` |
| full `Gossip` merge / tombstone / seen digest | core/membership | core contract 実装済み | `GossipStateModel`、`GossipStateSnapshot`、`GossipTombstone`、`GossipSeenDigest` が deterministic full-state merge、tombstone suppression / retention prune、seen digest convergence を扱う。tests: `full_state_merge_*`, `tombstone_*`, `seen_digest_*` |
| dedicated cluster heartbeat protocol | core/membership | core protocol 実装済み | `HeartbeatProtocolState`、`HeartbeatRequest`、`HeartbeatResponse`、`HeartbeatEvidence` が sequence number、request/response 照合、first / regular timeout evidence を扱う。tests: `heartbeat_tick_generates_sequence_per_peer`, `heartbeat_request_roundtrip_produces_reachable_evidence`, `first_and_regular_heartbeat_timeouts_are_observable` |
| `CrossDcClusterHeartbeat` | core/membership | core evidence 実装済み | `CrossDcHeartbeat`、`CrossDcHeartbeatRequest`、`CrossDcHeartbeatResponse`、`CrossDcHeartbeatEvidence` が data center pair 付き cross-DC liveness evidence と target change を扱う。tests: `cross_dc_*` |

この completed table は `cluster-gossip-heartbeat-protocol` の evidence だけを表す。SeedNodeProcess / generic discovery adapter は discovery provider interop の completed table に分離し、downing / SBR decision model と PubSub mediator / topic registry gossip はそれぞれ専用の completed table に分離する。次の責務は未完了のまま downstream / future scope に残す: SBR runtime actor、provider からの実 down execution loop、concrete lease coordination backend、versioned transport handoff / serde-postcard envelope bytes、Deferred Pekko concepts。

### Completed active follow-up: downing / SBR decision model

| 項目 | 実装先層 | 状態 | 根拠 / evidence |
|------|----------|------|-----------------|
| Downing evaluation input | core/downing_provider | core contract 実装済み | `DowningDecisionContext` が membership snapshot、reachability evidence、evaluation time、explicit down input を保持し、evidence 不足時の defer reason を生成する。tests: `downing_decision_context::*` |
| `DowningStrategy` / decision model | core/downing_provider | core decision model 実装済み | `DowningStrategyDecision` と `DowningDecisionTrace` が keep/down/defer/all-down、retained partition、downing targets、tie-break、stable-after、down-all timeout、lease outcome を保持する。tests: `downing_strategy_decision::*` |
| `SplitBrainResolver` | core/downing_provider | core evaluator 実装済み | `SplitBrainResolver` が KeepMajority、StaticQuorum、KeepOldest、DownAll、LeaseMajority を `DowningDecisionContext` から評価し、member state を変更せず trace 付き decision を返す。tests: `split_brain_resolver::*` |
| lease-based majority | core/downing_provider + std/provider | port contract + std binding 実装済み | `LeaseMajorityPort` / `LeaseAcquisitionOutcome` と `StdLeaseMajorityBackend` が lease outcome を core vocabulary へ変換し、`StdSplitBrainResolverProvider` が lifecycle 内で backend を所有する。tests: `lease_majority_port`, `lease_acquisition_outcome`, `split_brain_resolver_provider` |
| provider-facing SBR integration | core/downing_provider + std/provider | provider binding 実装済み | `SplitBrainResolverProviderHook` が compatibility metadata と decision/error mapping を提供し、`StdSplitBrainResolverProvider` が start/stop/drop lifecycle で hook と backend adapter を管理する。tests: `split_brain_resolver_provider_hook`, `split_brain_resolver_provider` |

この completed table は `cluster-downing-sbr-decision-model` の evidence だけを表す。SeedNodeProcess / generic discovery adapter と PubSub mediator / topic registry gossip はそれぞれ専用の completed table に分離する。次の責務は未完了のまま downstream / future scope に残す: SBR runtime actor、責任ノード選択、reachability 変化監視、provider からの実 down execution loop、concrete lease coordination backend、Deferred Pekko concepts。

### Completed active follow-up: discovery provider interop

| 項目 | 実装先層 | 状態 | 根拠 / evidence |
|------|----------|------|-----------------|
| `SeedNodeProcess` | core/cluster_provider + std/provider | boundary contract 実装済み | `SeedNodeInput` / `SeedNodeProcess` が seed authority を provider-neutral join input に変換し、empty seed、self filtering、duplicate seed、invalid authority、client start、shutdown 後停止を扱う。`ProviderLifecycleBridge` が seed input を topology update に接続する。tests: `seed_node_process_*`, `provider_lifecycle_bridge_*` |
| generic discovery adapter | core/cluster_provider + std/provider | boundary contract 実装済み | `DiscoveredAuthority` / `DiscoveryResult` / `DiscoveryTopologyMapper` と `DiscoveryBackend` / `DiscoveryBackendError` / `GenericDiscoveryAdapter` が backend result を provider-neutral topology input に正規化する。tests: `discovery_result_*`, `discovery_topology_mapper_*`, `generic_discovery_adapter_*`, `static_cluster_provider`, `aws_ecs` feature tests |

この completed table は `cluster-discovery-provider-interop` の SeedNodeProcess と generic discovery adapter evidence だけを表す。gossip heartbeat / full Gossip、reachability / WeaklyUp、downing / SBR decision model、PubSub mediator / topic registry gossip はそれぞれ専用 completed table の evidence に留め、この feature の完了根拠には含めない。次の責務は未完了のまま downstream / future scope に残す: SBR runtime actor、provider からの実 down execution loop、concrete lease coordination backend、Deferred Pekko concepts。

### Completed active follow-up: cluster message serialization contract

| 項目 | 実装先層 | 状態 | 根拠 / evidence |
|------|----------|------|-----------------|
| cluster message serializer contract | core/message_serialization + std/message_wire | contract evidence 実装済み | core contract は `ClusterMessagePayloadKind`、`ClusterMessageManifest`、`ClusterSerializedMessage`、`ActorSerializationBridge` で actor-core `SerializationExtension` と std/wire を橋渡しする。std adaptor は `ClusterWireFrameV1`、`ClusterWireCodec`、`ClusterWireDecodeFailure` で versioned frame と decode failure を扱う。tests: actor-core metadata preservation、unknown payload/version/malformed payload failure、gossip wire smoke、pubsub wire smoke |

この completed table は `cluster-message-serialization-contract` の serializer bridge evidence だけを表す。gossip merge / heartbeat / reachability semantics、pubsub mediator state / delivery / registry semantics、remote transport lifecycle、Pekko/protobuf 完全バイナリ互換は scope 外のまま残す。

### Completed active follow-up: distributed pubsub / topic registry

| 項目 | 実装先層 | 状態 | 根拠 / evidence |
|------|----------|------|-----------------|
| `DistributedPubSubMediator` protocol | core/pub_sub + std | protocol evidence 実装済み | `MediatorCommand`、`MediatorAcknowledgement`、`MediatorQueryResult`、`DistributedPubSubMediatorState` が subscribe / publish / unsubscribe / query を registry と delivery intent へ接続し、std `PubSubDeliveryIntentExecutor` が intent を実行する |
| `DistributedPubSubSettings` | core/pub_sub | settings contract 実装済み | `DistributedPubSubSettings` が role、routing mode、gossip interval、removed TTL、max delta elements、no-subscriber behavior を保持する |
| topic registry gossip / delta collection | core/pub_sub + membership | gossip payload evidence 実装済み | `TopicRegistryStatus`、`TopicRegistryDelta`、`TopicRegistryDeltaCollector`、`TopicRegistryGossipPayload`、`PubSubGossipHandoff` が owner version status、bounded delta、logical pubsub gossip kind を提供する |
| `Send` / `SendToAll` path semantics | core/pub_sub + actor-core | path semantics 実装済み | `MediatorPathKey` と `PubSubPathSemantics` が address-less canonical path、one-of、local affinity、all-of、all-but-self、no-subscriber intent を提供する |

この completed table は core pubsub mediator と topic registry gossip evidence だけを表す。Pekko `ClusterClient` / receptionist との統合、Distributed Data Replicator への寄せ込み、Pekko/protobuf 完全バイナリ互換は Deferred Pekko concepts または別 scope のまま残す。

### Active comparison follow-up: medium

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `PrepareForFullClusterShutdown` command path | core/typed + std | カテゴリ1 / カテゴリ5 |

### Active comparison follow-up: easy

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| failure detector implementation choice config | core/failure_detector + config | カテゴリ2 |

### Deferred Pekko concepts: trivial / easy

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `Flag` CRDT | core/ddata | カテゴリ9 |
| `Key[T]` / consistency levels | core/ddata | カテゴリ9 |
| `GCounter` / `PNCounter` / `PNCounterMap` | core/ddata | カテゴリ9 |
| std `ClusterApi` wrapper parity | std/api | カテゴリ10 |

### Deferred Pekko concepts: medium

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `SingletonActor[M]` | core/typed | カテゴリ6 |
| `ClusterSingletonSettings` | core/config | カテゴリ6 |
| `ClusterSingletonProxy` | std + core | カテゴリ6 |
| `ClusterReceptionistSettings` | core/config | カテゴリ6 |
| classic `ClusterSharding.start/startProxy` API | core/grain + std | カテゴリ8 |
| typed `ClusterSharding` extension | core/typed | カテゴリ8 |
| `Entity[M, E]` / `EntityContext` | core/typed + grain | カテゴリ8 |
| `EntityTypeKey[M]` / typed `EntityRef[M]` | core/typed + grain | カテゴリ8 |
| `ShardingEnvelope` / `ShardingMessageExtractor` | core/grain | カテゴリ8 |
| external shard allocation | core/placement + std | カテゴリ8 |
| `ReplicatedData` trait family | core/ddata | カテゴリ9 |
| `VersionVector` | core/ddata | カテゴリ9 |
| `LWWRegister` / `LWWMap` | core/ddata | カテゴリ9 |
| `ORSet` / `ORMap` / `ORMultiMap` | core/ddata | カテゴリ9 |
| typed DistributedData API | core/typed | カテゴリ9 |

### Deferred Pekko concepts: hard

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| typed `ClusterSingleton` extension | core/typed + std | カテゴリ6 |
| classic `ClusterSingletonManager` | std + core | カテゴリ6 |
| `ClusterClient` | std | カテゴリ6 |
| `ClusterClientReceptionist` | std + pub_sub | カテゴリ6 |
| shard allocation / rebalance strategy | core/placement | カテゴリ8 |
| remembered entities | core/placement + persistence integration | カテゴリ8 |
| `ShardedDaemonProcess` | core/typed + placement | カテゴリ8 |
| replicated sharding / direct replication | core/typed + placement | カテゴリ8 |
| sharding delivery controllers | core/typed + actor-core/delivery | カテゴリ8 |
| `DistributedData` extension | core + std | カテゴリ9 |
| `Replicator` / `ReplicatorSettings` | core + std | カテゴリ9 |

## 内部モジュール構造ギャップ

今回は API / 実動作ギャップが支配的であり、内部モジュール構造ギャップの詳細分析は省略する。Pekko comparison の固定スコープ概念カバレッジは約 60% で、hard / medium gap も多い。責務分割の細部比較より先に、Grain runtime の公開契約と end-to-end runtime を閉じる段階である。

次版で構造分析へ進む場合の観点は以下になる。

| 構造観点 | 現状 | 次に見るべき点 |
|----------|------|----------------|
| membership と provider の境界 | pure coordinator と provider/event-stream adapter が分かれている | SeedNodeProcess / discovery / downing がどちらに入るべきか |
| gossip と wire の境界 | core gossip / heartbeat contract + std logical handoff + existing postcard delta UDP + actor-core serialization bridge | Pekko/protobuf 完全バイナリ互換を将来採用するか |
| grain と typed sharding の境界 | protoactor-go style の Grain API が中心 | Pekko typed sharding wrapper を薄く載せられるか |
| pubsub と distributed-data の境界 | PubSub は独自 broker、CRDT は未実装 | PubSub registry gossip を ddata Replicator 相当に寄せるか |
| singleton / client の配置 | 対応モジュールなし | cluster-tools 相当を core contract と std actor runtime にどう分けるか |

## まとめ

cluster は membership、gossip / heartbeat contract、downing/SBR decision model、typed Cluster facade、Grain/Placement/Identity、PubSub、std UDP gossip transport、cluster message serializer contract という fraktor-rs 独自の基礎は強い。一方で、Pekko comparison の固定スコープ全体としては SBR runtime actor / down execution loop、singleton/client/receptionist、Distributed Data/CRDT、Pekko sharding public API が大きく未実装で、現時点の比較カバレッジは中程度である。

Pekko 概念を将来採用するなら、低コストで comparison gap を縮めやすいのは、`PrepareForFullClusterShutdown` command、基本 CRDT、std `ClusterApi` wrapper の再公開である。join config compatibility の checker composition、membership/reachability model、gossip/heartbeat protocol の core contract、cluster message serializer contract は契約化済み。router role/max-per-node 設定は、Pekko public API parity ではなく現行 router contract の拡張として実装済み。ただし、残りの実装には個別の OpenSpec change が必要であり、現在の Grain runtime roadmap の直近優先度とは分けて扱う。

主要な comparison gap は、Split Brain Resolver runtime、cluster singleton/client、sharding rebalance/remembered entities、Distributed Data Replicator、Pekko/protobuf serializer 完全バイナリ互換である。内部構造比較は、将来これらの scope を採用する OpenSpec change が立った後に進めるのが妥当である。
