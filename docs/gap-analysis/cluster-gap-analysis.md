# cluster モジュール ギャップ分析

更新日: 2026-07-10 (Grain runtime roadmap に基づく実装分類と実装済み判定を再整合)

## 位置づけ

fraktor-rs の `cluster-*` は、Proto.Actor-Go 型の Virtual Actor / Grain runtime を主軸にする。実装対象と順序の正本は [2026-05-25_cluster-grain-runtime-roadmap.md](../plan/2026-05-25_cluster-grain-runtime-roadmap.md) と個別の OpenSpec change である。

この文書は Pekko との差分を比較証跡として保持するが、parity 完了計画や実装バックログとしては使わない。比較上の未対応項目は、Grain runtime への必要性に基づいて「現行主軸」「Deferred」「対象外」に分類する。Pekko に型が存在することや実装難易度が低いことだけでは、着手理由にならない。

## 比較スコープ定義

cluster membership と virtual actor / sharding 相当の分散配置契約を対象にし、JVM 実装技術、Java/Scala DSL convenience、testkit、`cluster-metrics` は parity 分母から除外する。Pekko submodule は `2dc8960074bfe269da1686609eb88663cb50ad8b` を参照した。

### 対象に含めるもの

| 領域 | fraktor-rs | Pekko 参照 |
|------|------------|------------|
| cluster core | `modules/cluster-core-kernel/src/` (`activation`, `cluster_provider`, `downing_provider`, `extension`, `failure_detector`, `grain`, `membership`, `message_serialization`, `outbound`, `pub_sub`, `topology`) | `references/pekko/cluster/src/main/scala/org/apache/pekko/cluster/` |
| typed cluster contract | `modules/cluster-core-typed/src/` | `references/pekko/cluster-typed/src/main/scala/` |
| sharding / virtual actor | `modules/cluster-core-kernel/src/grain/`, `modules/cluster-core-kernel/src/activation/` | `references/pekko/cluster-sharding/`, `references/pekko/cluster-sharding-typed/` |
| cluster tools | `modules/cluster-core-kernel/src/pub_sub/`, `modules/cluster-core-kernel/src/singleton.rs`, `modules/cluster-core-kernel/src/singleton/`, `modules/cluster-core-typed/src/cluster_singleton_config.rs` | `references/pekko/cluster-tools/src/main/scala/org/apache/pekko/cluster/pubsub/`, `singleton/` |
| distributed data | `modules/cluster-core-kernel/src/ddata.rs`, `modules/cluster-core-kernel/src/ddata/` | `references/pekko/distributed-data/src/main/scala/org/apache/pekko/cluster/ddata/` |
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

fraktor-rs 側はスキル指定の `pub` 系抽出で、型 368 件 (core-kernel: 330, core-typed: 12, std: 26)、公開メソッド 1098 件 (core-kernel: 950, core-typed: 57, std: 91)。この数には `pub(crate)` の helper も含まれる参考値であり、parity カバレッジ分母には使わない。Pekko 側 raw 抽出（型宣言 857 件 / 主要 `def` 3027 件）も同様に参考値。core-kernel の raw 型数は 2026-06-16 版の 321 から 330 へ増え、sharding state-store-mode join compatibility の公開 config surface 追加と整合する。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象公開契約グループ | 151 |
| fraktor-rs で比較証跡を確認できる公開契約グループ | 117 |
| Pekko 比較一致率（進捗指標には使わない） | 117/151 (77%) |
| 比較上の部分対応 | 11 |
| 比較上の未対応 | 23（カテゴリ9の未対応 protocol 行は複数の公開契約グループを1行に集約） |
| raw public type declarations | 368 (core-kernel: 330, core-typed: 12, std: 26) |
| raw public method declarations | 1098 (core-kernel: 950, core-typed: 57, std: 91) |
| hard / medium / easy / trivial gap | 10 / 18 / 6 / 0 |
| panic 系スタブ | 0 件 |
| 機能 placeholder / TODO | 0 件 |

注: `raw public` は `pub(crate)` など内部到達可能な `pub` を含む参考値であり、crate 外から到達可能な外部公開 API 数ではない。
注: `比較証跡あり` / `部分対応` / `未対応` は、この台帳で定義した公開契約グループ単位で数える。ギャップ表の行数と概念グループ数は一致しない。カテゴリ9の protocol / CRDT 行は複数の公開契約グループを1行に集約している。raw 型名やメソッド名の個数を個別加算するものではない。

### 実装済み判定

roadmap 上の実装済み判定には、次の証跡を要求する。

- 利用者から到達可能な公開エントリポイント
- core policy と必要な adaptor まで接続された実行経路
- validation 後に所有 runtime が実際に消費する設定
- 公開エントリポイントから観測可能な state / effect / event までを通す contract test

純粋なデータ契約や decision model は、その公開 API 自体が完結した実行境界であり、law / state transition の contract test がある場合に限って単独で実装済みと扱う。型、config、wrapper、setup、installer が存在するだけの場合や、runtime を名乗る型が core を保持・検証するだけの場合は未実装である。

### 再検証ログ (2026-06-18, sharding join compatibility)

本版はカテゴリ10の `JoinConfigCompatCheckSharding` 相当だけを現行ツリーで再検証した。Pekko 参照は `JoinConfigCompatCheckSharding.scala` の required key `pekko.cluster.sharding.state-store-mode` と join rejection spec を証跡にした。

- `ClusterShardingStateStoreMode` と `ClusterExtensionConfig::with_sharding_state_store_mode` / `sharding_state_store_mode` により、cluster config が sharding state-store-mode 相当の Cluster Operational Contract を所有する。
- `ClusterCompatibilityKeyCatalog::SHARDING_STATE_STORE_MODE` と `ClusterExtensionConfig::required_join_compatibility_keys()` に `cluster.sharding.state-store-mode` が required key として現れる。
- `ClusterExtensionConfig::check_join_compatibility` の `JOIN_COMPATIBILITY_CHECKS` が sharding state-store-mode を走査し、不一致時は `cluster.sharding.state-store-mode mismatch: state_store_mode` を返す。
- `MembershipCoordinator::handle_join` は既存の join refusal 経路でこの `ConfigValidation::Incompatible` を `MembershipError::IncompatibleConfig` へ変換する。`join_rejects_incompatible_sharding_state_store_mode` で確認済み。
- `cluster.sharding.identity-lookup.choice` / `cluster.sharding.identity-lookup.tuning` は引き続き excluded key として維持する。identity lookup の factory 注入と local tuning は、required 比較へ昇格していない。
- raw 型宣言数は現行 grep で core-kernel 330 / core-typed 12 / std 26、raw 公開メソッド数は core-kernel 950 / core-typed 57 / std 91。

### 再検証ログ (2026-06-16, 全カテゴリ新証跡)

2026-06-16 版は前版の数値・判定を、当時の現行 HEAD（作業ツリー clean、Pekko submodule `2dc8960`）のソースから新規に証跡を取って再検証した。4 領域（ddata / sharding+singleton / membership+typed+std / Pekko スコープ）を独立に確認した結果:

- カテゴリ 1〜10 の実装済み / 部分実装 / 未対応の判定はすべて現行ツリーと一致（相違・幽霊エントリなし）。
- 実装本体の `todo!()` / `unimplemented!()` スタブは 0 件を再確認（テスト内 `panic!` は期待動作のアサーション）。
- raw 型宣言数のみ core-kernel が 314 → 321（+7）。増分は ddata の Replicator protocol core / `VersionVector` 系 / `LWWRegister` 系 / observed-remove CRDT 群の新規公開型で、カテゴリ9 の実装済み拡大と整合する（parity 分母外の参考値であり判定に影響しない）。公開メソッド数 1058（910/57/91）はスキル指定正規表現で再計測し一致。
- Pekko スコープの除外判定（ClusterClient 系の全面 `@deprecated` / SBR `DowningStrategy`・`SplitBrainResolverSettings` の `@InternalApi private[sbr]`・公開 SPI `SplitBrainResolverProvider` / typed `ClusterReceptionist` の `@InternalApi` / `cluster-metrics` 別ディレクトリ / `RemoveInternalClusterShardingData` migration utility）はすべて現行 Pekko ソースと整合。
- 表現修正: カテゴリ10 の `JoinConfigCompatCheckSharding` は Pekko 側で `@InternalApi` 実装クラスのため、parity 対象を「公開 SPI `JoinConfigCompatChecker` に sharding 必須 config key を寄与させる挙動」として記述し直した（2026-06-16 時点では機能ギャップ自体は維持）。

### 前回 (2026-06-05) からの判定変更

| 項目 | 旧判定 | 新判定 | 根拠 |
|------|--------|--------|------|
| std `ClusterApi` wrapper parity | 部分実装（get/request/down のみ） | 実装済み | `ClusterApi`（core-kernel）に `join` / `leave` / `subscribe` / `subscribe_no_replay` / `unsubscribe` / `current_state` / `self_authority` / `remote_path_of` / `down` / `get` / `request` / `request_future` がフルセットで存在。std は core API を直接使う設計で独自 wrapper は不要 |
| membership-driven routee add/remove | 実装済み (core policy) | 実装済み | core policy（`ClusterRouterPool::update_from_members`）に加え、std `ClusterRouterPoolRouteeSubscriber` が ClusterEvent 購読 → routee 更新を接続 |
| `SubscriptionInitialStateMode` | 分母外 | 実装済み | `ClusterSubscriptionInitialStateMode` が extension モジュールに存在 |
| pub-sub query | mediator protocol に包含 | 実装済み | `MediatorQuery::CurrentTopics` / `SubscriberCount` / `Count` を実装済み。`Count` は active owner の delivery view からトピック横断 subscriber 登録総数を返す |
| `MembershipCoordinatorDriver` | 実装済み概念として列挙 | 公開型ではない | `pub(super)`。`TokioGossiper` の内部実装であり外部 API には露出しない |
| SBR runtime down execution loop | 部分実装 | 実装済み | `SplitBrainResolverProviderHook::decide_strategy_context` / `StdSplitBrainResolverProvider::decide_strategy_context` が target-aware decision を保持し、std `SplitBrainResolverDowningDriver` + `TokioGossiper::with_split_brain_resolver_downing` が reachability snapshot から downing target を `MembershipCoordinator::handle_down` と `ClusterProvider::down` へ自動発行 |
| `VersionVector` | 未対応 | 実装済み | `VersionVector` が per-node causal ordering / merge / removed-node pruning を実装し、`VersionVectorOrdering` と property tests で CRDT merge law を検証 |
| `LWWRegister` | 未対応 | 実装済み | `LWWRegister<T>` が signed timestamp / `UniqueAddress` ordering / clock-valid same-writer timestamp contract による last-writer-wins merge、caller-supplied current millis を使う default clock と negated supplied millis による reverse clock、clock closure 経由の値更新を実装し、`LWWRegisterKey<T>` と merge law / tie-break tests で検証 |
| `ClusterShardingHealthCheck` | 未対応 | 実装済み | `GrainReadinessSnapshot::readiness` が self node status / placement state / registered kinds から pure に `GrainReadiness` を導出し、`ClusterExtension::grain_readiness_snapshot` が呼び出し時点の入力 snapshot を公開 |
| `JoinConfigCompatCheckSharding` | 部分実装 | 実装済み | `ClusterShardingStateStoreMode` を `ClusterExtensionConfig` が所有し、`cluster.sharding.state-store-mode` required key と `JOIN_COMPATIBILITY_CHECKS` 経由で mismatch reason を生成する。`MembershipCoordinator` の join refusal 経路までテスト済み |

備考: `ClusterPubSub` trait は `pub_sub::cluster_pub_sub::ClusterPubSub` のネストパスでのみ公開されており、トップレベル `pub_sub` への re-export はない（実装済み判定は維持、公開面の整理は別件）。`NodeStatus` の Pekko `Down` 相当は `Dead` バリアント（別名実装済み）。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core / membership | `Cluster`, `Member`, `MemberStatus`, `CurrentClusterState`, `ClusterEvent`, `Gossip`, `Reachability` | `ClusterExtension`, `ClusterApi`, `NodeRecord`, `NodeStatus`, `CurrentClusterState`, `MembershipCoordinator`, `GossipDisseminationCoordinator`, `ReachabilityMatrix`, `GossipStateModel`, `HeartbeatProtocolState`, `CrossDcHeartbeat` | UniqueAddress、data center、WeaklyUp、reachability matrix、gossip envelope、full gossip merge / tombstone / seen digest、dedicated heartbeat evidence、SeedNodeProcess の core contract はある。Member ordering 公開契約、shutdown 系イベント型、CoordinatedShutdown 連携が不足 |
| core / downing | `DowningProvider`, `NoDowning`, `SplitBrainResolverProvider` | `DowningProvider`, `DowningDecisionContext`, `DowningStrategyDecision`, `DowningDecisionTrace`, `NoopDowningProvider`, `SplitBrainResolver`, `LeaseMajorityPort`, `SplitBrainResolverProviderHook` | decision model / settings / provider binding / target-aware runtime decision は完了。std 側の自動 down 発行ループも `TokioGossiper` opt-in で接続済み。concrete lease backend が未実装 |
| core / typed | typed `Cluster`, command, subscription, singleton, sharding typed API | `Cluster` / `ClusterCommand` / `ClusterStateSubscription` / `ClusterEventSubscription` / `SelfUp` / `SelfRemoved` / `ClusterSetup` | typed Cluster facade（subscribe / unsubscribe / current_state / `PrepareForFullClusterShutdown` command 含む）は完備。singleton / sharding typed API が未実装 |
| core / virtual actor | `ClusterSharding`, `EntityRef`, `EntityTypeKey`, `ShardRegion`, coordinator | `GrainRef`, `GrainKey`, `GrainTypeKey`, typed `GrainRef`, `ShardingEnvelope`, `ShardingMessageExtractor`, `ShardingRouter`, `VirtualActorRegistry`, `PlacementCoordinatorCore`, `PartitionIdentityLookup`, `RendezvousHasher`, `PidCache`, `GrainReadinessSnapshot` | protoactor-go style の同等機能は強いが、Pekko public API 形態（typed Entity / `ClusterSharding.init` / `EntityRef` / `askWithStatus`）、rebalance / remembered entities / query protocol が不足 |
| core / distributed state | `DistributedData`, `Replicator`, CRDT 型群 | `ReplicatedData`, `DeltaReplicatedData`, `RemovedNodePruning`, `Key`, `SelfUniqueAddress`, `Flag`, `GCounter`, `PNCounter`, `PNCounterMap`（increment / decrement / get / entries / remove）, `VersionVector`, `LWWRegister`, `ORSet`, `ORMap`, `ORMultiMap`, `LWWMap`, read/write consistency 語彙, `Get` / `Update` / `Delete` / `Subscribe` protocol core | CRDT 基底 SPI と scalar counter 型、`PNCounterMap` の entries surface / observed-remove key deletion、`VersionVector` の causal ordering / merge / pruning、`LWWRegister` の timestamp / node-order / clock-valid same-writer timestamp contract / negated-time reverse clock、observed-remove set/map/multimap と LWW map、Replicator command / response / local entry evaluation protocol は実装済み。DistributedData / Replicator runtime、durable store、typed adapter が不足 |
| std / adapter | gossip transport, provider, discovery adapter | `TokioGossipTransport`, `TokioGossiper`, `LocalClusterProvider`, `StaticClusterProvider`, `AwsEcsClusterProvider`, `GenericDiscoveryAdapter`, `ProviderLifecycleBridge`, `ClusterWireCodec`, `ConfiguredPhiAccrualDetectorFactory` | Rust adapter、logical envelope handoff、seed/discovery provider boundary、cluster message serializer contract、failure detector factory、SBR down execution loop はある。sharding state-store-mode の required 比較は core config 経路で接続済み。sharding/singleton 系 setup が不足 |

## カテゴリ別ギャップ

各カテゴリのヘッダーに **比較証跡を確認できる数 / 対象公開契約グループ数 (比較一致率)** を明記する。差分（未対応・部分対応）のみテーブルに列挙し、比較証跡を確認できる契約は件数カウントに含めてテーブル行には追加しない。この数値は実装優先度や roadmap 進捗を表さない。

### 1. Cluster membership / lifecycle — 比較証跡 20/22 (91%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `ClusterScope` deploy scope | `ClusterActorRefProvider.scala:148` | 未対応 | core/extension | medium | cluster-aware deployment scope 概念がない。router config はあるが deploy scope としての統合はない |
| `ClusterSettings.CrossDcFailureDetectorSettings` / `MultiDataCenter` | `ClusterSettings.scala:65`, `ClusterSettings.scala:76` | 未対応 | core/config | easy | `CrossDcHeartbeat` evidence と `FailureDetectorConfig` はあるが、Multi-DC 専用の failure detector 設定 namespace がない |

比較証跡を確認できるもの: cluster extension、join/leave/down（`ClusterApi` フルセット）、event stream subscription、current state snapshot、member/up/removed callback、roles/app_version 設定、leader/role leader 算出、startup/shutdown event、`prepare_for_full_cluster_shutdown` command path（`MemberStatusChanged` → `MemberPreparingForShutdown` 発火）、`CoordinatedShutdownLeave` hook（`CoordinatedShutdown::PHASE_CLUSTER_LEAVE` + `ClusterExtensionInstaller` による `ClusterApi::leave(self_authority)` task 登録）、`UniqueAddress` semantics（`NodeRecord::unique_address` / `try_join_with_identity`）、data center membership、`WeaklyUp`、`remotePathOf`、`MemberStatus` 全 variant（`Down` ≈ `Dead` 別名実装済み）、`PreparingForShutdown` / `ReadyForShutdown` status、`ClusterSettings` 契約（`ClusterExtensionConfig` + `FailureDetectorConfig` + `ConfigValidation`）、`JoinConfigCompatChecker` + `ConfigValidation`、Member ordering 公開契約（`member_age_order` / `age_ordered` / `oldest_member`、2026-06-11 cluster-membership-event-surface）、`ClusterLogMarker` 相当の構造化 tracing field 契約（`cluster_lifecycle_trace_field` + std `ClusterLifecycleLogSubscriber`、同上）。

### 2. Gossip / reachability / failure detection — 比較証跡 18/18 (100%)

このカテゴリの未対応ギャップは解消済み（2026-06-11 cluster-membership-event-surface で `MemberPreparingForShutdown` / `MemberReadyForShutdown` イベント variant + coordinator 併発、`UnreachableDataCenter` / `ReachableDataCenter` イベント + `DataCenterReachabilityTable` ラッチを実装）。

比較証跡を確認できるもの: `Reachability` matrix（`ReachabilityMatrix` / `ReachabilityRecord` / `ReachabilitySnapshot`）、full `Gossip` merge / tombstone / seen digest（`GossipStateModel` / `GossipStateSnapshot`）、`GossipEnvelope` + logical handoff、dedicated heartbeat protocol（`HeartbeatProtocolState`）、`CrossDcClusterHeartbeat` evidence（`CrossDcHeartbeat`）、`SeedNodeProcess`、config compatibility full key set（`ClusterCompatibilityKeyCatalog` / `JoinCompatibilityComposition`）、Failure Detector Configuration（`FailureDetectorConfig` + `ConfiguredPhiAccrualDetectorFactory`）、`SubscriptionInitialStateMode`（`ClusterSubscriptionInitialStateMode`）、`MembershipTable` / `MembershipDelta` / `MembershipVersion` / `VectorClock`、`DefaultFailureDetectorRegistry`、`MembershipCoordinator::poll` による suspect/dead 遷移、indirect connection evidence、`TokioGossipTransport`。

### 3. Downing / Split Brain Resolver — 比較証跡 4/5 (80%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| concrete lease coordination backend | `DowningStrategy.scala:602` | 部分実装 | std | hard | `LeaseMajorityPort` / `LeaseAcquisitionOutcome` / `StdLeaseMajorityBackend` trait は実装済み。実際の分散 lease backend（coordination service 連携、retry、network I/O）が未実装 |

比較証跡を確認できるもの: `DowningProvider` SPI、`NoDowning`（`NoopDowningProvider`）、SBR settings 契約（`SplitBrainResolverConfig` / `SplitBrainResolverStrategy`: KeepMajority / StaticQuorum / KeepOldest / DownAll / LeaseMajority の 5 戦略）、SBR runtime down execution loop（`SplitBrainResolverProviderHook::decide_strategy_context` / `StdSplitBrainResolverProvider::decide_strategy_context` / std `SplitBrainResolverDowningDriver` / `TokioGossiper::with_split_brain_resolver_downing` / `MembershipCoordinator::handle_down`）。`DowningDecisionContext` / `DowningStrategyDecision` / `DowningDecisionTrace` / `FailureObservation` / `IndirectConnectionEvidence` / downing provider・SBR settings の join compatibility は上記概念の evidence。

### 4. Cluster router pool / group — 比較証跡 6/6 (100%)

このカテゴリの未対応ギャップは解消済み。

比較証跡を確認できるもの: `ClusterRouterPool`、`ClusterRouterGroup`、pool/group settings の分離、`use_roles` / `satisfies_roles`、`max_instances_per_node`、`ClusterRouterPool::from_candidates` の least-loaded 配置、std `ClusterRouterPoolRouteeSubscriber` による `ClusterEvent` 購読 → routee 更新。

### 5. Cluster Typed API — 比較証跡 14/14 (100%)

このカテゴリの未対応ギャップは解消済み。

比較証跡を確認できるもの: typed `Cluster` facade、`ClusterStateSubscription` 契約、Subscribe（`Cluster::subscribe` / `subscribe_self_up` / `subscribe_self_removed`）、Unsubscribe（`Cluster::unsubscribe`）、GetCurrentState（`Cluster::current_state`）、`ClusterCommand` sealed 契約と Join / JoinSeedNodes / Leave / Down / PrepareForFullClusterShutdown、`SelfUp`、`SelfRemoved`、typed Cluster extension（`Cluster::get`）、`ClusterSetup`。

### 6. Cluster singleton — 比較証跡 3/10 (30%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| classic `ClusterSingletonManager` | `ClusterSingletonManager.scala:492` | 未対応 | std + core | hard | oldest-node election、handover protocol、termination message が必要 |
| `ClusterSingletonProxy` | `ClusterSingletonProxy.scala:171` | 未対応 | std + core | medium | singleton location 追跡と proxy 送信 / buffering がない |
| typed `ClusterSingleton` extension | `cluster-typed/ClusterSingleton.scala:135` | 未対応 | core/typed + std | hard | cluster 全体で一つの actor を保証する typed extension がない |
| `SingletonActor[M]` | `cluster-typed/ClusterSingleton.scala:153` | 未対応 | core/typed | medium | singleton entity 設定 wrapper がない |
| typed `ClusterSingletonManagerSettings` | `cluster-typed/ClusterSingleton.scala:223` | 部分実装 | core/config | easy | typed 専用名の manager settings 型はないが、`ClusterSingletonConfig::to_manager_config` で manager 設定へ導出できる |
| `ClusterSingletonSetup` | `cluster-typed/ClusterSingleton.scala:326` | 未対応 | core/typed + std | easy | ActorSystem setup 統合がない |
| `ClusterSingletonManagerIsStuck` 検知契約 | `ClusterSingletonManager.scala`（exception/failure 契約） | 部分実装 | core | easy | `SingletonStuckPhase` と `ClusterEvent::SingletonHandOverStuck` の観測語彙はあるが、runtime 検知ループはない |

比較証跡を確認できるもの: `ClusterSingletonManagerSettings` 相当（`ClusterSingletonManagerConfig`）、`ClusterSingletonProxySettings` 相当（`ClusterSingletonProxyConfig`）、typed `ClusterSingletonSettings` 相当（`ClusterSingletonConfig` から manager / proxy 設定への導出）。これらは設定・validation 契約の比較証跡であり、singleton runtime の実装済み判定ではない。

n/a へ移動: `ClusterClient` / `ClusterClientReceptionist` / `ClusterClientSettings` / `ClusterReceptionistSettings`（Pekko 本体で全面 `@deprecated`、gRPC 移行推奨）。typed `ClusterReceptionist` は `@InternalApi`（receptionist の公開契約は actor-typed 側スコープ）。

### 7. Distributed PubSub — 比較証跡 11/11 (100%)

このカテゴリの未対応ギャップは解消済み。

比較証跡を確認できるもの: `DistributedPubSubMediator` protocol（`MediatorCommand` / `MediatorAcknowledgement` / `DistributedPubSubMediatorState`）、`DistributedPubSubConfig`、topic registry gossip / delta collection（`TopicRegistryStatus` / `TopicRegistryDelta` / `TopicRegistryDeltaCollector` / `PubSubGossipHandoff`）、`Send` / `SendToAll` path semantics（`MediatorPathKey` / `PubSubPathSemantics`）、`DistributedPubSub` extension 相当（`ClusterPubSub` trait / `ClusterPubSubImpl` / `ClusterPubSubShared`）、Subscribe / Unsubscribe / Publish メッセージ、`GetTopics` / `CurrentTopics`（`MediatorQuery::CurrentTopics`）、`CountSubscribers`（`MediatorQuery::SubscriberCount`）、mediator 全体 `Count`（`MediatorQuery::Count`）、`DistributedPubSubMessage` marker 相当（`ClusterMessagePayloadKind` / `PubSubEnvelope`）、broker / delivery（`PubSubBroker` + std `PubSubDeliveryActor` / `PubSubDeliveryIntentExecutor`）。

### 8. Sharding / Grain / Placement / Identity — 比較証跡 14/27 (52%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| classic `ClusterSharding.start/startProxy` API | `ClusterSharding.scala:224`, `ClusterSharding.scala:516` | 部分実装 | core/grain + std | medium | `setup_member_kinds` / `GrainRef` はあるが Pekko 風 start/startProxy API（proxy-only mode 含む）はない |
| typed `ClusterSharding` extension | `typed/scaladsl/ClusterSharding.scala:40` | 部分実装 | core/typed | medium | grain API はあるが typed extension 形態の `init(Entity)` API ではない |
| `Entity[M, E]` / `EntityContext` | `typed/scaladsl/ClusterSharding.scala:238`, `typed/scaladsl/ClusterSharding.scala:363` | 部分実装 | core/typed + grain | medium | `ActivatedKind` / `GrainContext` は対応するが typed behavior factory ではない |
| `EntityTypeKey[M]` / typed `EntityRef[M]`（ask / askWithStatus 含む） | `typed/scaladsl/ClusterSharding.scala:407`, `typed/scaladsl/ClusterSharding.scala:429` | 部分実装 | core/typed + grain | easy | `GrainTypeKey<M>` / typed `GrainRef<M>` と typed request / future はあるが、Pekko 形態の `EntityRef` API と `askWithStatus` 統合はない |
| shard allocation / rebalance strategy | `ShardCoordinator.scala:110`, `ShardCoordinator.scala:295` | 部分実装 | core/placement | hard | rendezvous hashing による placement はあるが、`ShardAllocationStrategy` SPI、`LeastShardAllocationStrategy` 相当の rebalance、coordinator handoff protocol がない |
| `ClusterShardingSettings`（classic + typed） | `ClusterShardingSettings.scala:32`, `typed/ClusterShardingSettings.scala:33` | 部分実装 | core/config | medium | `GrainCallOptions` / `PartitionIdentityLookupConfig` 等の個別設定はあるが、包括的な sharding settings 契約がない |
| sharding query protocol（`GetShardRegionState` / `GetShardRegionStats` / `GetClusterShardingStats` / `GetCurrentRegions` + 応答型、classic + typed） | `ShardRegion.scala:237-386`, `ClusterShardingQuery.scala:39` | 未対応 | core/grain + core/typed | medium | shard / region / entity 数の observability query がない（`GrainMetrics` は別系統の metrics） |
| passivation strategy settings（idle / LRU / MRU / LFU / admission） | `ClusterShardingSettings.scala:243` | 未対応 | core/config + grain | medium | passivation 自体はあるが、strategy 設定階層（active entity limit、segmented LRU、admission window / filter）がない |
| remembered entities（`RememberEntitiesStore` / `StateStoreMode` / `RememberEntitiesStoreMode`） | `RememberEntitiesStore.scala:57`, `ClusterShardingSettings.scala:125` | 未対応 | core/placement + persistence integration | hard | activation registry はあるが、再起動 / rebalance 後にエンティティを再活性化する store 契約がない |
| external shard allocation（extension / strategy / client / `ShardLocations`） | `ExternalShardAllocation.scala:32`, `ExternalShardAllocationStrategy.scala:44` | 未対応 | core/placement + std | medium | 外部から shard 配置を指定する API がない |
| `ShardedDaemonProcess` / `ShardedDaemonProcessSettings` | `ShardedDaemonProcess.scala:30`, `ShardedDaemonProcessSettings.scala:27` | 未対応 | core/typed + placement | hard | N 個の daemon を shard 配置し keep-alive する API がない |
| replicated sharding（`ReplicatedShardingExtension` / `ReplicatedSharding` / `ReplicatedEntityProvider` / `ReplicatedEntity`） | `ReplicatedShardingExtension.scala:31`, `ReplicatedEntityProvider.scala:32` | 未対応 | core/typed + placement | hard | data center / replica id model がない |
| sharding delivery controllers（`ShardingProducerController` / `ShardingConsumerController`） | `ShardingProducerController.scala:104`, `ShardingConsumerController.scala:50` | 未対応 | core/typed + actor-core/delivery | hard | reliable delivery と sharding の接続がない |

比較証跡を確認できるもの: `GrainRef`、`GrainKey`、typed `GrainTypeKey<M>`、typed `GrainRef<M>`、`GrainCodec`、`ShardingEnvelope`、`ShardingMessageExtractor` SPI、Pekko 互換 `HashCodeMessageExtractor` / `HashCodeNoEnvelopeMessageExtractor`、Kafka 互換 `Murmur2MessageExtractor`、`ShardingRouter`、`VirtualActorRegistry`、`PlacementCoordinatorCore`、`PartitionIdentityLookup`、`RendezvousHasher`、`PidCache`、remote/local placement decision、passivation（基本機構）、RPC router（`GrainRpcRouter`）、`ClusterShardingHealthCheck` 相当の grain readiness 契約（`GrainReadinessSnapshot` / `GrainReadiness` / `GrainUnreadyReason` + `ClusterExtension::grain_readiness_snapshot`）。

### 9. Distributed Data / CRDT — 比較証跡 17/27 (63%)

このカテゴリの `17/27` は raw 型数ではなく、(1) CRDT 基底 SPI、(2) Key 階層、(3) SelfUniqueAddress、(4) scalar state / counter CRDT 群（`Flag` / `GCounter` / `PNCounter`）、(5) read/write consistency 語彙、(6) 補助 protocol 語彙、(7) `PNCounterMap` の entries surface / observed-remove key deletion、(8) `VersionVector` の causal ordering / merge / removed-node pruning、(9) `LWWRegister` の timestamp / node-order / clock-valid same-writer timestamp contract、(10) Get protocol、(11) Update protocol、(12) Subscribe protocol、(13) Delete protocol、(14) `ORSet`、(15) `ORMap`、(16) `ORMultiMap`、(17) `LWWMap`、という公開契約グループ単位で数える。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `DistributedData` extension (classic) | `DistributedData.scala:27` | 未対応 | core + std | hard | replicator extension がない |
| typed `DistributedData` extension | `cluster-typed/ddata/typed/scaladsl/DistributedData.scala:33` | 未対応 | core/typed | medium | typed extension wrapper がない |
| `ReplicatorMessageAdapter[A, B]` | `cluster-typed/ddata/typed/scaladsl/ReplicatorMessageAdapter.scala:27` | 未対応 | core/typed | medium | typed Behavior と replicator protocol の連携 adapter がない |
| `Replicator` / `ReplicatorSettings` | `Replicator.scala:73`, `Replicator.scala:162` | 未対応 | core + std | hard | gossip-based CRDT replication 基盤（write/read repair、delta propagation、pruning 実行体）がない |
| `DurableStore` SPI（`Store` / `LoadAll` / `LoadData` protocol） | `DurableStore.scala:64-86` | 未対応 | core/ddata | medium | durable storage の port 契約がない |
| durable store std adapter（`LmdbDurableStore` 相当） | `DurableStore.scala:112` | 未対応 | std | medium | LMDB 完全互換ではなく、embedded KV による std 実装が対象 |

比較証跡を確認できるもの: CRDT merge / delta / pruning 基底 SPI（`ReplicatedData` / `DeltaReplicatedData` / `ReplicatedDelta` / `RequiresCausalDeliveryOfDeltas` / `RemovedNodePruning`）、`Key<T>` と基本 key alias、`SelfUniqueAddress`、`Flag`、`GCounter`、`PNCounter`、`PNCounterMap`（increment / decrement / get / entries / remove、delta / pruning）、`VersionVector`（increment / compare / merge / removed-node pruning）、`LWWRegister`（signed timestamp / `UniqueAddress` ordering / clock-valid same-writer timestamp contract / caller-supplied current millis default clock / negated supplied millis reverse clock update）、`ORSet`（add-wins observed-remove set / delta / pruning）、`ORMap`（observed-remove key set + recursive CRDT value merge / delta / pruning）、`ORMultiMap`（`ORMap<A, ORSet<B>>` による binding add/remove / delta / pruning）、`LWWMap`（`ORMap<A, LWWRegister<B>>` による timestamped entry merge / delta / pruning）、read/write consistency 語彙（`ReadConsistency` / `WriteConsistency`）、補助 protocol 語彙（`GetReplicaCount` / `ReplicaCount` / `FlushChanges`）、Replicator protocol core（`ReplicatorEntry` に対する `Get::respond_from`、`Update::evaluate`、`Delete::evaluate`、`Subscribe` / `Unsubscribe` と `SubscribeResponse`）。純粋な CRDT value と protocol 評価の証跡であり、Replicator runtime の実装済み判定ではない。

### 10. std adapter / discovery / wire integration — 比較証跡 10/11 (91%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| module setup integration（`ClusterShardingSetup` / `ClusterSingletonSetup` 相当） | `cluster-sharding-typed/scaladsl/ClusterSharding.scala:541`, `cluster-typed/ClusterSingleton.scala:326` | 未対応 | core/typed + std | easy | sharding / singleton extension 自体が未実装のため従属的に未対応 |

比較証跡を確認できるもの: cluster message serializer contract（`ClusterMessagePayloadKind` / `ClusterMessageManifest` / `ClusterSerializedMessage` / `ActorSerializationBridge` + std `ClusterWireFrameV1` / `ClusterWireCodec` / `ClusterWireDecodeFailure`）、seed node discovery process（`SeedNodeProcess` + `ProviderLifecycleBridge`）、generic discovery adapter（`DiscoveryBackend` / `GenericDiscoveryAdapter` / AWS ECS feature）、`ClusterApi` 公開面 parity（join / leave / subscribe / unsubscribe / current_state / down / get / request / remote_path_of のフルセット — 判定変更）、transport lifecycle bridge retention（`subscribe_remoting_events`）、gossip transport adapter（`TokioGossipTransport` / `TokioGossiper`）、provider lifecycle（`LocalClusterProvider` / `StaticClusterProvider` / `AwsEcsClusterProvider`）、versioned wire frame、discovery topology mapper、sharding 必須 config key の join 互換比較（`ClusterShardingStateStoreMode` / `ClusterCompatibilityKeyCatalog::SHARDING_STATE_STORE_MODE` / `ClusterExtensionConfig::check_join_compatibility` / `MembershipCoordinator::handle_join`）。

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

今回は API / 実動作ギャップが支配的なため、内部モジュール構造ギャップの詳細分析は省略する。固定スコープ概念カバレッジは 77% で、判定基準（カバレッジ 80% 以上、または hard/medium 未実装 5 件以下）を満たさない。

次版で構造分析へ進む場合の観点:

| 構造観点 | 現状 | 次に見るべき点 |
|----------|------|----------------|
| membership と provider の境界 | pure coordinator と provider/event-stream adapter が分かれている | SBR runtime loop は std `TokioGossiper` の opt-in driver として接続済み。CoordinatedShutdownLeave は actor-core phase と cluster extension installer hook に接続済み |
| gossip と wire の境界 | core gossip / heartbeat contract + std logical handoff + postcard delta UDP + actor-core serialization bridge | Pekko/protobuf 完全バイナリ互換を将来採用するか |
| grain と typed sharding の境界 | protoactor-go style の Grain API が中心 | typed sharding wrapper は Deferred。現行 Grain API の実行経路を深く保つ |
| pubsub と distributed-data の境界 | PubSub は独自 broker、CRDT 基本型はあるが Replicator は未実装 | Replicator 統合は Deferred。具体的な registry replication 要件が出るまで独立を維持する |
| singleton の配置 | core-kernel に singleton 設定 / error / stuck phase 語彙、core-typed に統合設定があるが、manager / proxy runtime と typed extension はない | runtime は Deferred。hidden singleton を導入せず、具体的な単一 owner 要件から再検討する |

## 実装分類

比較上の差分は、難易度ではなく Grain runtime roadmap への適合性で分類する。分類が変わる場合は、先に roadmap または個別 OpenSpec change で理由を確定する。

### 現行主軸

| 領域 | 対象となる変更 | 完了条件 |
|------|----------------|----------|
| Membership / topology | join / leave / down、到達性、rolling update 時の topology と placement cache の一貫性 | 公開 API から membership state / event / placement までを contract test で確認できる |
| Failure detector / downing | Availability Evidence、最小 downing decision、既存 provider への実行接続 | config、decision、std 実行経路が同じ slice で閉じる |
| Grain identity / placement | identity lookup、Rendezvous placement、activation / idle passivation | config または command が lookup / coordinator / registry で消費され、event と metrics まで観測できる |
| Provider boundary | local / static / discovery provider と cluster core の責務分離 | provider input から topology update までを integration test で確認できる |

現行主軸の変更でも、Pekko の名前や設定階層を追加すること自体は目的にしない。たとえば idle passivation の設定接続は対象になり得るが、同時に LRU / MRU / LFU や remembered entities を追加しない。

### Deferred

次の領域は比較証跡として差分を残すが、具体的な Grain runtime 上の要求と個別 OpenSpec change ができるまで実装しない。

- typed Cluster / ClusterSharding wrapper、`Entity` / `EntityContext`、sharding query / setup
- Cluster Singleton / ShardCoordinator / handover parity
- Distributed Data の Replicator runtime、durable store、typed adapter
- least-shard rebalance、remembered entities、external shard allocation
- LRU / MRU / LFU / admission を含む高度な passivation policy
- sharded daemon、replicated sharding、sharding delivery controller
- concrete lease backend、in-flight request draining

Deferred 領域では、config、protocol、actor wrapper、installer、in-memory adapterだけを先行追加しない。純粋な CRDT value や decision model のように単独で完結する契約を拡張する場合も、既存の下流要求または承認済み spec を必要とする。

### 対象外

次の差分は現在の実装計画に含めない。

- Grain API で既に満たしている挙動に対する classic `ClusterSharding.start/startProxy` などのAPI形状だけの移植
- 利用する runtime がない包括的な `ClusterShardingSettings` や deploy `ClusterScope`
- ClusterClient 系、`@InternalApi` 型、Java / Scala DSL convenience
- JMX / HOCON / JFR / classloader、testkit、protobuf完全バイナリ互換、migration utility

## バックログ運用

- Issue は利用者から見える end-to-end behavior を一つだけ所有し、core / adaptor / tests を水平分割しない。
- Acceptance criteria には公開エントリポイント、runtime での消費点、観測結果、contract test を含める。
- Pekko API 名だけを追加する Issue、複数の無関係な helper を束ねる Issue、設定や wrapper だけの Issue は作成しない。
- 既存の parity 中心 Issue は、現行主軸の vertical slice へ書き直せない場合はクローズする。必要になった時点で新しい要求から Issue を作る。

### 再検証ログ (2026-07-10, Grain runtime roadmap 再整合)

- raw 型数は core-kernel 330 / core-typed 12 / std 26、公開メソッド数は 950 / 57 / 91 で、2026-06-18 版から変化していない。
- Pekko 比較グループは 151、比較証跡あり 117、部分対応 11、未対応 23 で、比較結果自体は変化していない。
- 117/151 は roadmap 進捗ではなく、比較証跡としてのみ保持する。
- [PR #2045](https://github.com/j5ik2o/fraktor-rs/pull/2045) で試みられた typed Cluster API、Cluster Singleton runtime、Replicator runtime、least-shard rebalance、remembered entities は main へ入っておらず、Deferred 判定を維持する。
- 型、config、wrapper、installer の存在だけでは実装済みとしない完了ゲートを roadmap と本書に追加した。

## まとめ

cluster の現行主軸は、membership / gossip / heartbeat / reachability / downing、Grain identity / placement / activation / idle passivation、provider boundary である。Pekko 比較一致率 117/151 は参考値であり、残り 34 契約を埋める計画ではない。

次の実装は、roadmap の成功条件に寄与する vertical slice から選ぶ。Deferred または対象外の項目は、難易度や Pekko 側の公開範囲を理由に着手しない。
