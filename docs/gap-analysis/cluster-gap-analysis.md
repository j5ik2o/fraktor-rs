# cluster モジュール ギャップ分析

更新日: 2026-04-24 (固定スコープ版)

## 比較スコープ定義

この調査は、Apache Pekko cluster 配下の raw API 数をそのまま移植対象にするものではない。fraktor-rs の `cluster` では、cluster membership と virtual actor / sharding 相当の分散配置契約を対象にし、JVM 実装技術、Java/Scala DSL convenience、testkit、`cluster-metrics` は parity 分母から除外する。

### 対象に含めるもの

| 領域 | fraktor-rs | Pekko 参照 |
|------|------------|------------|
| cluster core | `modules/cluster-core/src/core/` | `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/` |
| typed cluster contract | 対応する `core/typed` は現状なし | `references/pekko/cluster-typed/src/main/scala/` |
| sharding / virtual actor | `modules/cluster-core/src/core/grain/`, `identity/`, `placement/` | `references/pekko/cluster-sharding/`, `references/pekko/cluster-sharding-typed/` |
| cluster tools | `modules/cluster-core/src/core/pub_sub/` | `references/pekko/cluster-tools/src/main/scala/org/apache/pekko/cluster/pubsub/`, `singleton/`, `client/` |
| distributed data | 対応モジュールなし | `references/pekko/distributed-data/src/main/scala/org/apache/pekko/cluster/ddata/` |
| std adapter | `modules/cluster-adaptor-std/src/std/` | gossip transport / provider / discovery adapter として Rust で再現可能な契約 |

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

固定スコープ対象ディレクトリを raw 抽出すると、Pekko 側は型宣言 844 件、主要 `def` 2995 件が見つかる。これには private / internal / JVM 固有 / DSL wrapper / serializer 実装が含まれるため、parity カバレッジ分母には使わない。

fraktor-rs 側はスキル指定の `pub` 系抽出で、型 187 件 (core: 174, std: 13)、公開メソッド 434 件 (core: 380, std: 54)。ただし、この数には `pub(crate)` の wire helper も含まれる。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 約 121 |
| fraktor-rs 固定スコープ対応概念 | 約 45 |
| 固定スコープ概念カバレッジ | 約 45/121 (37%) |
| hard gap | 18 |
| medium gap | 26 |
| easy gap | 20 |
| trivial gap | 4 |
| panic 系スタブ | 0 件 |
| 機能 placeholder / TODO | 1 件 |

cluster は、membership table、gossip dissemination、failure detector registry、Grain/Placement/Identity、PubSub broker、UDP gossip transport などの基礎部品はかなり揃っている。一方で固定スコープ全体で見ると、typed cluster API、Split Brain Resolver、cluster singleton/client、Pekko sharding の public API、Distributed Data/CRDT が大きく未実装である。

旧版は raw Scala 宣言数をサマリーに置きつつ、`cluster-metrics` を混ぜ、`ShardedDaemonProcess` や typed API を YAGNI で n/a にしていた。固定スコープ版では、JVM 固有以外の public runtime contract は parity ギャップとして扱う。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core / membership | `Cluster`, `Member`, `MemberStatus`, `CurrentClusterState`, `ClusterEvent`, `Gossip`, `Reachability` | `ClusterExtension`, `ClusterApi`, `NodeRecord`, `NodeStatus`, `CurrentClusterState`, `MembershipCoordinator`, `GossipDisseminationCoordinator` | 基本契約はあるが data center、weakly-up、reachability matrix、seed process が不足 |
| core / downing | `DowningProvider`, `NoDowning`, SBR | `DowningProvider`, `NoopDowningProvider` | Noop と明示 down hook だけ。SBR は未実装 |
| core / typed | typed `Cluster`, command, subscription, singleton, sharding typed API | `core/typed` なし | 未実装 |
| core / virtual actor | `ClusterSharding`, `EntityRef`, `EntityTypeKey`, `ShardRegion`, coordinator | `GrainRef`, `GrainKey`, `VirtualActorRegistry`, `PlacementCoordinatorCore`, `PartitionIdentityLookup` | protoactor-go style の同等機能は強いが Pekko public API と remember/rebalance が不足 |
| core / distributed state | `DistributedData`, `Replicator`, CRDT 型群 | なし | 未実装 |
| std / adapter | gossip transport, provider, discovery adapter | `TokioGossipTransport`, `MembershipCoordinatorDriver`, `LocalClusterProvider`, `AwsEcsClusterProvider` | Rust adapter はあるが seed / discovery / wire integration は限定的 |

## カテゴリ別ギャップ

### 1. Cluster membership / lifecycle　✅ 実装済み 12/17 (71%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `UniqueAddress` semantics | `Member.scala:315`, `Member.scala:331` | 部分実装 | core/membership | medium | `NodeRecord` は authority と node_id を持つが、address + UID の一意性モデルではない |
| data center membership | `Cluster.scala:102`, `ClusterEvent.scala:396` | 未対応 | core/membership | medium | `NodeRecord` に data center がない。Cross-DC event もない |
| `WeaklyUp` / full member status compatibility | `Member.scala:241`, `ClusterEvent.scala:279` | 部分実装 | core/membership | easy | `NodeStatus` は基本状態を持つが `WeaklyUp` 相当がない |
| `prepareForFullClusterShutdown` | `Cluster.scala:336`, `cluster-typed/Cluster.scala:175` | 部分実装 | core + std | medium | `PreparingForShutdown` / `ReadyForShutdown` は型だけあり、full shutdown command path がない |
| `remotePathOf` | `Cluster.scala:442` | 未対応 | core or actor-core integration | easy | cluster API から remote actor path を返す helper がない |

実装済みとして扱うもの: cluster extension、join/leave/down、event stream subscription、current state snapshot、member/up/removed callback、roles/app_version 設定、leader/role leader 算出、startup/shutdown event。

### 2. Gossip / reachability / failure detection　✅ 実装済み 7/15 (47%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `Reachability` matrix | `Reachability.scala:36`, `Reachability.scala:38` | 部分実装 | core/membership | medium | fraktor は `Suspect` と unreachable event で表現。observer/subject/version の matrix がない |
| full `Gossip` merge / tombstone / seen digest | `Gossip.scala:127`, `Gossip.scala:178`, `Gossip.scala:230` | 部分実装 | core/membership | hard | `GossipDisseminationCoordinator` は delta diffusion 中心。tombstone prune、full merge、seen digest が不足 |
| `GossipEnvelope` | `Gossip.scala:307` | 部分実装 | core/membership + std/wire | medium | `GossipOutbound` はあるが from/to `UniqueAddress` と lazy serialization deadline がない |
| dedicated `ClusterHeartbeatSender` / receiver protocol | `ClusterHeartbeat.scala:82`, `ClusterHeartbeat.scala:90` | 部分実装 | std + core/membership | medium | `handle_heartbeat` はあるが sequence number / response / first heartbeat expectation はない |
| `CrossDcClusterHeartbeat` | `CrossDcClusterHeartbeat.scala:230` | 未対応 | core/membership + std | hard | data center model が未実装のため未対応 |
| `SeedNodeProcess` | `SeedNodeProcess.scala:22` | 部分実装 | std/provider | medium | `LocalClusterProvider::with_seed_nodes` はあるが InitJoin/JoinSeedNode プロセスはない |
| config compatibility full key set | `JoinConfigCompatChecker.scala:25` | 部分実装 | core/config | easy | `ClusterExtensionConfig` の TODO は pubsub 設定だけ検査し、roles/app_version/gossip_config を未検査 |
| failure detector implementation choice | `Cluster.scala:124`, `Cluster.scala:131` | 部分実装 | core/failure_detector | easy | registry はあるが cluster config から deadline/phi などを選ぶ設定 contract がない |

実装済みとして扱うもの: `MembershipTable`、`MembershipDelta`、`MembershipVersion`、`VectorClock`、`DefaultFailureDetectorRegistry`、`MembershipCoordinator::poll` による suspect/dead 遷移、`TokioGossipTransport`。

### 3. Downing / Split Brain Resolver　✅ 実装済み 2/8 (25%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `SplitBrainResolver` | `SplitBrainResolver.scala:50`, `SplitBrainResolver.scala:160` | 未対応 | core + std | hard | stable-after、責任ノード、reachability 変化監視、down 実行がない |
| `DowningStrategy` / decision model | `DowningStrategy.scala:28`, `DowningStrategy.scala:342` | 未対応 | core/downing_provider | hard | KeepMajority / StaticQuorum / KeepOldest / LeaseMajority の判定モデルがない |
| `SplitBrainResolverSettings` | `SplitBrainResolverSettings.scala:39` | 未対応 | core/config | easy | SBR 設定型がない |
| `SplitBrainResolverProvider` | `SplitBrainResolverProvider.scala` | 未対応 | std/provider | easy | provider factory がない |
| lease-based majority | `DowningStrategy.scala:602` | 未対応 | core + std | hard | lease abstraction / coordination integration がない |
| indirect connection handling | `DowningStrategy.scala:245` | 未対応 | core/membership | medium | reachability matrix 不足の影響で判定不能 |

実装済みとして扱うもの: `DowningProvider` trait、`NoopDowningProvider`、明示 `ClusterApi::down` hook。

### 4. Cluster router pool / group　✅ 実装済み 3/6 (50%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| role-filtered routee selection | `ClusterRouterConfig.scala:80`, `ClusterRouterConfig.scala:190` | 部分実装 | core/router | easy | config に `useRoles` 相当がない |
| max instances per node | `ClusterRouterConfig.scala:190` | 未対応 | core/router | easy | pool config は total_instances だけ |
| membership-driven routee add/remove | `ClusterRouterConfig.scala:586`, `ClusterRouterConfig.scala:591` | 部分実装 | core/router + event integration | medium | routee selection type はあるが ClusterEvent 連携で自動更新する runtime がない |

実装済みとして扱うもの: `ClusterRouterPool`、`ClusterRouterGroup`、pool/group settings の分離。

### 5. Cluster Typed API　✅ 実装済み 0/7 (0%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| typed `Cluster` extension | `cluster-typed/Cluster.scala:186`, `cluster-typed/Cluster.scala:202` | 未対応 | core/typed | medium | `modules/cluster-core/src/core/typed/` が存在しない |
| `ClusterCommand` | `cluster-typed/Cluster.scala:82` | 未対応 | core/typed | easy | Join / JoinSeedNodes / Leave / Down / shutdown command enum 相当 |
| `ClusterStateSubscription` | `cluster-typed/Cluster.scala:34`, `cluster-typed/Cluster.scala:45` | 未対応 | core/typed | easy | typed actor ref subscriber wrapper がない |
| `SelfUp` | `cluster-typed/Cluster.scala:65` | 未対応 | core/typed | trivial | `MemberStatusChanged` から導出可能 |
| `SelfRemoved` | `cluster-typed/Cluster.scala:73` | 未対応 | core/typed | trivial | `MemberStatusChanged` から導出可能 |
| `ClusterSetup` | `cluster-typed/Cluster.scala:225` | 未対応 | core/typed | easy | typed ActorSystem setup hook がない |
| `PrepareForFullClusterShutdown` command | `cluster-typed/Cluster.scala:175` | 未対応 | core/typed + std | medium | core lifecycle command と coordinated shutdown 接続が必要 |

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

### 7. Distributed PubSub　✅ 実装済み 6/10 (60%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `DistributedPubSubMediator` protocol | `DistributedPubSubMediator.scala:151`, `DistributedPubSubMediator.scala:553` | 部分実装 | core/pub_sub + std | medium | `PubSubBroker` はあるが mediator actor protocol と registry gossip がない |
| `DistributedPubSubSettings` | `DistributedPubSubMediator.scala:44`, `DistributedPubSubMediator.scala:103` | 部分実装 | core/pub_sub | easy | `PubSubConfig` は TTL 等が限定的。role/routing/maxDeltaElements がない |
| topic registry gossip / delta collection | `DistributedPubSubMediator.scala:699`, `DistributedPubSubMediator.scala:861` | 未対応 | core/pub_sub + membership | hard | topic/subscriber registry を cluster gossip へ載せる処理がない |
| `Send` / `SendToAll` path semantics | `DistributedPubSubMediator.scala:206`, `DistributedPubSubMediator.scala:216` | 部分実装 | core/pub_sub + actor-core | medium | topic publish はあるが actor path への direct send semantics が不足 |

実装済みとして扱うもの: `ClusterPubSub` trait、`ClusterPubSubImpl`、`PubSubBroker`、topic / subscriber / publish ack、delivery policy、partition behavior、std `PubSubDeliveryActor`。

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

### 10. std adapter / discovery / wire integration　✅ 実装済み 4/9 (44%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| cluster message serializer contract | `ClusterMessageSerializer.scala:83`, `ClusterShardingMessageSerializer.scala`, `DistributedPubSubMessageSerializer.scala` | 部分実装 | std/wire + actor-core serialization | hard | gossip delta は postcard wire だが Pekko cluster/sharding/pubsub message serializer に相当する contract がない |
| seed node discovery process | `SeedNodeProcess.scala:22` | 部分実装 | std/provider | medium | seed list は保持できるが active join orchestration がない |
| generic discovery adapter | `Cluster.scala:354`, `ClusterClient.scala:65` | 部分実装 | std/provider | medium | static/local/AWS ECS はあるが discovery provider abstraction は限定的 |
| std `ClusterApi` wrapper parity | `Cluster.scala:328`, `Cluster.scala:384`, `Cluster.scala:395` | 部分実装 | std/api | trivial | std wrapper は `get/request/down` のみで `join/leave/subscribe` を再公開していない |
| transport lifecycle to membership bridge retention | `local_cluster_provider_ext.rs` | 部分実装 | std/provider | easy | subscription を保持しないため、購読 lifetime が provider と連動している保証が弱い |

実装済みとして扱うもの: `TokioGossipTransport`、`MembershipCoordinatorDriver`、`LocalClusterProvider`、`StaticClusterProvider`、`AwsEcsClusterProvider`。

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

`todo!()` / `unimplemented!()` / `panic!("not implemented")` は検出されなかった。

| 箇所 | 種別 | 備考 |
|------|------|------|
| `modules/cluster-core/src/core/cluster_extension_config.rs:134` | TODO | join config compatibility が pubsub 設定だけで、gossip_config / app_version / roles の検査が未実装 |

## 実装優先度

### Phase 1: trivial / easy

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `SelfUp` / `SelfRemoved` | core/typed | カテゴリ5 |
| `ClusterCommand` | core/typed | カテゴリ5 |
| `ClusterStateSubscription` | core/typed | カテゴリ5 |
| `ClusterSetup` | core/typed | カテゴリ5 |
| `SplitBrainResolverSettings` | core/config | カテゴリ3 |
| `SplitBrainResolverProvider` | std/provider | カテゴリ3 |
| role-filtered router config | core/router | カテゴリ4 |
| max instances per node | core/router | カテゴリ4 |
| `Flag` CRDT | core/ddata | カテゴリ9 |
| `Key[T]` / consistency levels | core/ddata | カテゴリ9 |
| `GCounter` / `PNCounter` / `PNCounterMap` | core/ddata | カテゴリ9 |
| std `ClusterApi` wrapper parity | std/api | カテゴリ10 |
| config compatibility full key set | core/config | カテゴリ2 |
| `remotePathOf` | core or actor-core integration | カテゴリ1 |

### Phase 2: medium

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `UniqueAddress` semantics | core/membership | カテゴリ1 |
| data center membership | core/membership | カテゴリ1 |
| `WeaklyUp` compatibility | core/membership | カテゴリ1 |
| `prepareForFullClusterShutdown` | core + std | カテゴリ1 |
| `Reachability` matrix | core/membership | カテゴリ2 |
| `GossipEnvelope` | core/membership + std/wire | カテゴリ2 |
| dedicated cluster heartbeat protocol | std + core/membership | カテゴリ2 |
| `SeedNodeProcess` | std/provider | カテゴリ2 |
| indirect connection handling | core/membership | カテゴリ3 |
| membership-driven router update | core/router + event integration | カテゴリ4 |
| typed `Cluster` extension | core/typed | カテゴリ5 |
| `SingletonActor[M]` | core/typed | カテゴリ6 |
| `ClusterSingletonSettings` | core/config | カテゴリ6 |
| `ClusterSingletonProxy` | std + core | カテゴリ6 |
| `ClusterReceptionistSettings` | core/config | カテゴリ6 |
| `DistributedPubSubMediator` protocol | core/pub_sub + std | カテゴリ7 |
| `DistributedPubSubSettings` | core/pub_sub | カテゴリ7 |
| `Send` / `SendToAll` path semantics | core/pub_sub + actor-core | カテゴリ7 |
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
| generic discovery adapter | std/provider | カテゴリ10 |
| transport lifecycle bridge retention | std/provider | カテゴリ10 |

### Phase 3: hard

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| full `Gossip` merge / tombstone / seen digest | core/membership | カテゴリ2 |
| `CrossDcClusterHeartbeat` | core/membership + std | カテゴリ2 |
| `SplitBrainResolver` | core + std | カテゴリ3 |
| `DowningStrategy` / decision model | core/downing_provider | カテゴリ3 |
| lease-based majority | core + std | カテゴリ3 |
| typed `ClusterSingleton` extension | core/typed + std | カテゴリ6 |
| classic `ClusterSingletonManager` | std + core | カテゴリ6 |
| `ClusterClient` | std | カテゴリ6 |
| `ClusterClientReceptionist` | std + pub_sub | カテゴリ6 |
| topic registry gossip / delta collection | core/pub_sub + membership | カテゴリ7 |
| shard allocation / rebalance strategy | core/placement | カテゴリ8 |
| remembered entities | core/placement + persistence integration | カテゴリ8 |
| `ShardedDaemonProcess` | core/typed + placement | カテゴリ8 |
| replicated sharding / direct replication | core/typed + placement | カテゴリ8 |
| sharding delivery controllers | core/typed + actor-core/delivery | カテゴリ8 |
| `DistributedData` extension | core + std | カテゴリ9 |
| `Replicator` / `ReplicatorSettings` | core + std | カテゴリ9 |
| cluster message serializer contract | std/wire + actor-core serialization | カテゴリ10 |

## 内部モジュール構造ギャップ

今回は API / 実動作ギャップが支配的であり、内部モジュール構造ギャップの詳細分析は省略する。固定スコープ概念カバレッジは約 37% で、hard / medium gap も多いため、責務分割の細部比較より先に公開契約と end-to-end runtime を閉じる段階である。

次版で構造分析へ進む場合の観点は以下になる。

| 構造観点 | 現状 | 次に見るべき点 |
|----------|------|----------------|
| membership と provider の境界 | pure coordinator と provider/event-stream adapter が分かれている | SeedNodeProcess / discovery / downing がどちらに入るべきか |
| gossip と wire の境界 | core delta + std postcard UDP | cluster message serializer contract を actor-core serialization に寄せるか |
| grain と typed sharding の境界 | protoactor-go style の Grain API が中心 | Pekko typed sharding wrapper を薄く載せられるか |
| pubsub と distributed-data の境界 | PubSub は独自 broker、CRDT は未実装 | PubSub registry gossip を ddata Replicator 相当に寄せるか |
| singleton / client の配置 | 対応モジュールなし | cluster-tools 相当を core contract と std actor runtime にどう分けるか |

## まとめ

cluster は membership、gossip delta、Grain/Placement/Identity、PubSub、std UDP gossip transport という fraktor-rs 独自の基礎は強い。一方で、Pekko cluster 固定スコープ全体としては typed cluster API、SBR、singleton/client/receptionist、Distributed Data/CRDT、Pekko sharding public API が大きく未実装で、現時点のカバレッジは中程度より低い。

低コストで parity を前進できるのは、typed cluster の薄い command/event wrapper、SBR 設定型、router role/max-per-node 設定、基本 CRDT、std `ClusterApi` wrapper の再公開、join config compatibility の拡張である。

主要ギャップは、Split Brain Resolver、cluster singleton/client、topic registry gossip、sharding rebalance/remembered entities、Distributed Data Replicator、cluster/sharding/pubsub serializer contract である。内部構造比較は、これらの API / 実動作ギャップを閉じた後に進めるのが妥当である。
