# remote モジュール ギャップ分析

更新日: 2026-04-29 (9th edition / main 最新化後再検証版)

## 比較スコープ定義

この分析は Apache Pekko `remote` の raw API 全体を移植対象にするものではない。fraktor-rs の `remote` では、Pekko Artery の責務分割と remote actor transport 契約を parity 対象にし、classic remoting、JVM 実装技術、testkit、Pekko wire byte compatibility は分母から除外する。

スキル定義では `remote-core/src/domain/` を core 相当とする記述があるが、現行 `modules/remote-core/src/lib.rs` は crate root から remote core modules を公開しており、`src/domain/` は存在しない。このため本レポートでは現行ツリーを優先し、`modules/remote-core/src/` を core 相当として扱う。

### 対象に含めるもの

| 領域 | fraktor-rs | Pekko 参照 |
|------|------------|------------|
| remote core | `modules/remote-core/src/` | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/` |
| Artery transport contract | `modules/remote-core/src/{transport,association,wire}/` | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/artery/` |
| std TCP adapter | `modules/remote-adaptor-std/src/{transport/tcp,association}/` | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/artery/tcp/` |
| remote actor ref provider | `modules/remote-core/src/provider/`, `modules/remote-adaptor-std/src/provider/` | `RemoteActorRefProvider.scala`, `RemoteActorRef` 相当 |
| failure detector / watcher | `modules/remote-core/src/{failure_detector,watcher}/`, `modules/remote-adaptor-std/src/watcher_actor/` | `FailureDetector*.scala`, `RemoteWatcher.scala` |
| serialization 接続点 | `modules/actor-core/src/core/kernel/serialization/`, `modules/remote-core/src/wire/` | `remote/serialization/` の remote transport に必要な契約 |
| lifecycle / instrumentation | `modules/remote-core/src/{extension,instrument,config}/`, `modules/remote-adaptor-std/src/extension_installer/` | `RemotingLifecycleEvent.scala`, `RemoteLogMarker.scala`, `RemoteInstrument.scala`, `RemoteSettings.scala`, `ArterySettings.scala` |

### 対象から除外するもの

| 除外項目 | 理由 |
|----------|------|
| classic remoting / `Endpoint.scala` / `AckedDelivery.scala` | Pekko 側でも deprecated。Artery 互換の分母には入れない |
| `transport/netty/`, `PekkoProtocolTransport.scala`, `PekkoPduCodec.scala`, `AbstractTransportAdapter.scala` | classic transport stack |
| `FailureInjectorTransportAdapter`, `ThrottlerTransportAdapter`, `TestTransport`, `TestStage.scala` | test / fault injection 用 |
| Aeron UDP transport (`artery/aeron/*`) | JVM Aeron 固有実装。Rust std TCP adapter とは別物 |
| TLS / `SSLEngineProvider` / `security/provider/*` | Java `SSLEngine` / HOCON / classloader に依存する完全互換は除外。Rust TLS adapter が必要なら別スコープ |
| `JavaSerializer` / Jackson module 完全互換 | serialization contract との接続点だけ対象 |
| HOCON provider loading / `FailureDetectorLoader` 動的ロード / JVM classloader | JVM 設定ロード方式に依存 |
| JFR `artery/jfr/Events.scala`, `JFRRemotingFlightRecorder.scala` | JVM 固有。Rust 側は `RemotingFlightRecorder` で代替 |
| remote testkit / multi-node-testkit / remote-tests | runtime API ではない |
| `RemoteMetricsExtension`, `AddressUidExtension`, `BoundAddressesExtension` | JVM 拡張ローダ依存。同等情報は `RemotingLifecycleState` / `RemoteAuthoritySnapshot` で再現済み |
| `EnvelopeBufferPool`, `ObjectPool`, `FixedSizePartitionHub` | JVM GC 回避目的の最適化用 buffer pool。Rust では割り当て戦略が異なる |
| `ImmutableLongMap`, `LruBoundedCache` | internal collection helper。Rust では `hashbrown` / `BTreeMap` / 専用 cache で代替 |
| `ProtobufSerializer` | Pekko 内部で protobuf wire encode を分離するための adapter。fraktor は現状独自 binary codec を採用 |

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 73 |
| fraktor-rs 固定スコープ対応概念 | 62 |
| 固定スコープ概念カバレッジ | 62/73 (85%) |
| raw Pekko public type declarations | 365 |
| raw Pekko `def` declarations | 1594 |
| raw fraktor public type declarations | 88 (`remote-core`: 63, `remote-adaptor-std`: 25) |
| raw fraktor public method declarations | 372 (`remote-core`: 276, `remote-adaptor-std`: 96) |
| hard / medium / easy / trivial gap | 9 / 2 / 0 / 0 |

raw declaration count は private / deprecated / JVM 固有 API を含む参考値であり、parity 分母には使わない。

8th edition からの主な更新:

| 領域 | 現行評価 |
|------|----------|
| concrete remote `ActorRef` construction | `StdRemoteActorRefProvider` が remote branch で `RemoteActorRefSender` を持つ `ActorRef` を materialize するため、ギャップから除外 |
| remote resolve cache | remote path の cache hit / miss が実際の `ActorRef` を返し、synthetic remote PID を再利用するため、実装済みとして扱う |
| `ActorIdentity` remote ActorRef restoration | `MiscMessageSerializer` が scheme 対応 provider を経由して remote path を復元するため、ギャップから除外 |
| `RemoteRouterConfig` routee expansion | `RoundRobinPool`、`SmallestMailboxPool`、`RandomPool`、`ConsistentHashingPool` の remote routee expansion は実装済み。任意 hash mapper の serialization は未対応として残す |
| advanced Artery settings | large-message destinations、outbound large-message queue、inbound restart budget、compression config の型 surface は実装済み。runtime 適用は残ギャップ |
| payload send | 空 payload placeholder ではなく fail-fast へ改善済み。ただし user payload serialization と `TcpRemoteTransport::send` は未完成 |
| inbound delivery | `WireFrame::Envelope` は受信できるが、local actor / mailbox へ配送しない状態のまま |

`todo!()` / `unimplemented!()` / `panic!("not implemented")` は remote core / adaptor の production code から検出されない。一方で Phase コメントは残っており、主に payload serialization、remote send、inbound actor delivery の未完成を示している。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core / remote primitives | address、unique address、association、wire PDU、failure detector、watcher state、provider contract、typed config | `modules/remote-core/src/` に整理済み。no_std 側の状態機械と PDU は揃っている | 公開 primitive は強い。残りは end-to-end runtime 接続 |
| std / adaptor | TCP listener/client、association runtime、remoting lifecycle、watcher actor、reconnect/backoff | `TcpRemoteTransport`、`AssociationRegistry`、`run_inbound_dispatch`、`run_outbound_loop_with_reconnect`、`WatcherActor` は存在 | bind / handshake / reconnect / quarantine filter は動く。message delivery は未完了 |
| actor-core integration | serialization registry、ActorRefProvider、DeathWatch、event stream、routing/deploy | misc serializer、scheme provider lookup、remote `ActorRef` materialization、routee expansion は接続済み | remote `send`、remote DeathWatch 通知、remote deployment が残る |

## カテゴリ別ギャップ

ギャップ表には未対応・部分実装・n/a のみを列挙する。実装済み項目はカテゴリ件数に含めるが、表には出さない。

### 1. Address / identity　✅ 実装済み 4/4 (100%)

`Address`, `UniqueAddress`, `RemoteNodeId`, `resolve_remote_address` は実装済み。Pekko の `UniqueAddress(address, uid)` と同じ責務を持つ。

### 2. Failure detector　✅ 実装済み 6/6 (100%)

`FailureDetector`, `DeadlineFailureDetector`, `PhiAccrualFailureDetector`, `HeartbeatHistory`, `FailureDetectorRegistry`, `DefaultFailureDetectorRegistry` は実装済み。address-bound detector registry も no_std core に入っている。

### 3. Transport / association / lifecycle　✅ 実装済み 16/17 (94%)

`Association`, `AssociationEffect`, `SendQueue`, `QuarantineReason`, `HandshakeValidationError`, `RemoteTransport`, `TcpRemoteTransport`, `AssociationRegistry`, `AssociationShared`, `HandshakeDriver`, `SystemMessageDeliveryState`, `ReconnectBackoffPolicy`, `RestartCounter`, `InboundQuarantineCheck`, lifecycle effect application は実装済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| inbound envelope delivery | `artery/MessageDispatcher.scala`, `artery/ArteryTcpTransport.scala` | 部分実装 | std/association + actor-core | hard | `run_inbound_dispatch` は `WireFrame::Envelope` を debug log するだけで local actor / mailbox へ配送しない |

### 4. Wire protocol / serialization　✅ 実装済み 12/13 (92%)

`FrameHeader`, `EnvelopePdu`, `HandshakePdu`, `ControlPdu`, `AckPdu` と各 codec、`MessageContainerSerializer`、`SystemMessageSerializer`、`MiscMessageSerializer` 主要 manifest、manifest-route fallback、`ActorIdentity` remote `ActorRef` restoration、outbound / inbound `maximum_frame_size` enforcement は実装済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| message payload serialization into envelope | `MessageSerializer.scala`, `ArteryMessageSerializer.scala` | 部分実装 | std/provider + std/transport/tcp + actor-core/serialization | hard | remote actor send 用に `AnyMessage` を serializer registry へ流し、`OutboundEnvelope` / `EnvelopePdu` へ載せる driver がない |

### 5. Provider / remote actor ref / routing　✅ 実装済み 7/11 (64%)

`RemoteActorRef`, `RemoteActorRefProvider` trait、local/no-authority dispatch、loopback authority dispatch、`ActorRefResolveCache` 経由の remote resolve、cache hit/miss event publish、concrete remote `ActorRef` construction は実装済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| remote send path | `RemoteActorRefProvider.scala`, `RemoteTransport.send` | 部分実装 | std/provider + std/transport/tcp | hard | `RemoteActorRefSender` は `ActorRef` に包まれるが、`send` は `remote payload serialization is not installed` で fail-fast する |
| remote DeathWatch interception | `RemoteActorRefProvider.scala`, `RemoteWatcher.scala` | 部分実装 | std/provider + std/watcher_actor + actor-core | hard | `watch` / `unwatch` intent は provider にあるが、actor-core DeathWatch への最終接続がない |
| consistent-hashing pool remote router serialization | `remote/routing/RemoteRouterConfig.scala`, `routing/ConsistentHashing.scala` | 部分実装 | actor-core/serialization | medium | `RemoteRouterConfig` は非ジェネリック化済みで runtime expansion も対応済み。任意 `hash_key_mapper` は wire に載せられないため serializer が明示的に NotSerializable を返す |
| remote deployment daemon / `useActorOnNode` | `RemoteDaemon.scala`, `RemoteDeployer.scala`, `RemoteDeploymentWatcher.scala` | 未対応 | std/provider + actor-core | hard | remote child actor 作成要求と deployment watcher がない |

### 6. Watcher / DeathWatch runtime　✅ 実装済み 5/7 (71%)

`WatcherState`, `WatcherCommand`, `WatcherEffect`, `WatcherActor`, `run_heartbeat_loop`, heartbeat response UID tracking、UID 変更時の rewatch effect は実装済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| watcher effects application | `RemoteWatcher.scala` | 部分実装 | std/watcher_actor + actor-core | hard | `WatcherActor` は effects を `effect_tx` へ流すだけで、`Terminated` / event stream / system message に適用しない |
| `AddressTerminated` integration | `RemoteWatcher.scala`, actor event stream | 未対応 | actor-core + std/watcher_actor | hard | remote node failure を local DeathWatch へ統合する契約がない |

### 7. Instrumentation / config / logging　✅ 実装済み 8/9 (89%)

`RemotingLifecycleState`, `StdRemoting`, `EventPublisher`, `RemoteLogMarker`, `RemoteInstrument`, `RemotingFlightRecorder`, `RemoteAuthoritySnapshot`、主要 `RemoteConfig` builder は実装済み。`bind_hostname` / `bind_port` / `inbound_lanes` / `outbound_lanes` / `maximum_frame_size` / `buffer_pool_size` / `untrusted_mode` / log toggle / outbound queue / remove-quarantined / outbound restart budget / inbound restart budget / large-message destinations / compression config は現行コードで確認済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| advanced Artery settings runtime application | `ArterySettings.scala`, `artery/compress/*` | 部分実装 | core/config + std/runtime | medium | 型 surface はあるが、large-message queue selection、compression advertisement / table application、config に基づく runtime path 分岐は未接続 |

### 8. Reliability / lifecycle adapter　✅ 実装済み 2/4 (50%)

`InboundQuarantineCheck` と `RestartCounter` は実装済み。shutdown / DeathWatch 前 flush は残る。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| `FlushOnShutdown` | `artery/FlushOnShutdown.scala` | 未対応 | std/extension_installer + std/association | hard | `shutdown_flush_timeout` 設定はあるが、association に termination hint を送り ack を待つ driver がない |
| `FlushBeforeDeathWatchNotification` | `artery/FlushBeforeDeathWatchNotification.scala` | 未対応 | std/watcher_actor + std/association | hard | DeathWatch 通知前に対象 association を flush する契約がない |

### 9. Internal helpers / cache　✅ 実装済み 2/2 (100%)

`ActorRefResolveCache` と `RemoteActorRefResolveCacheEvent` / `RemoteActorRefResolveCacheOutcome` は実装済み。`StdRemoteActorRefProvider` から hit/miss event も publish される。

## 対象外（n/a）

| Pekko API / 領域 | 判定理由 |
|------------------|----------|
| classic remoting `Endpoint*`, `AckedDelivery`, `PekkoProtocolTransport`, `PekkoPduCodec`, `transport/Transport.scala` | deprecated classic remoting |
| `transport/netty/*`, `FailureInjectorTransportAdapter`, `ThrottlerTransportAdapter`, `TestTransport` | classic transport / fault injection / test 用 |
| Aeron UDP transport (`artery/aeron/{ArteryAeronUdpTransport,AeronSink,AeronSource,TaskRunner}`) | JVM Aeron 固有 |
| `SSLEngineProvider`, `ConfigSSLEngineProvider`, `RotatingKeysSSLEngineProvider`, `security/provider/*` | Java `SSLEngine` 完全互換は対象外。Rust TLS adapter が必要なら別スコープ |
| `JavaSerializer` / Jackson module 完全互換 | serializer contract との接続点だけ対象 |
| `RemoteMetricsExtension`, `AddressUidExtension`, `BoundAddressesExtension` | JVM 拡張ローダ依存。同等情報は `RemotingLifecycleState` / `RemoteAuthoritySnapshot` で再現 |
| `EnvelopeBufferPool`, `ObjectPool`, `FixedSizePartitionHub` | JVM GC 回避用 buffer pool |
| `ImmutableLongMap`, `LruBoundedCache` | internal collection helper |
| `ProtobufSerializer` | Pekko 内部の protobuf bridge。fraktor は独自 binary codec |
| Pekko Artery TCP framing byte compatibility | B 方針により対象外。fraktor は独自 `length(4) + version(1) + kind(1)` framing を維持する |
| `ArteryMessageSerializer` protobuf control protocol compatibility | B 方針により対象外。handshake / control / ack の責務は fraktor 独自 PDU で閉じる |
| `DaemonMsgCreateSerializer` byte compatibility | B 方針により対象外。remote deployment 責務が必要な場合も fraktor 独自 serializer として扱う |
| Artery compression table wire compatibility | B 方針により対象外。compression が必要な場合も fraktor 独自 wire 上の責務として扱う |
| `artery/jfr/Events.scala`, `JFRRemotingFlightRecorder.scala` | JVM Flight Recorder 固有。Rust 側は `RemotingFlightRecorder` で代替 |
| HOCON provider loading / `FailureDetectorLoader` 動的ロード / JVM classloader | JVM 設定ロード方式 |
| `TestStage`, multi-node-testkit, remote-tests | runtime API ではない |

## 方針判断

Phase 3 hard の実装方針は、以下の判断により `responsibility parity` に固定する。

### 決定. Artery responsibility parity と wire-level compatibility のどちらを採用するか

この判断は、Pekko Artery のどの層を parity 対象にするかを決めるものである。fraktor-rs は Artery 相当の remote actor transport を目指すが、それは Pekko ノードと wire-level で相互運用することを意味しない。

| 層 | 方針 |
|----|------|
| remote actor transport の責務 | Artery を参考にする |
| association / handshake / quarantine / DeathWatch などの概念 | Artery 相当を目指す |
| 設定項目の意味 | Artery に寄せる |
| TCP framing の byte layout | fraktor 独自 |
| protobuf control PDU | 採用しない |
| Pekko ノードとの wire 相互運用 | 目指さない |
| compression table の wire 表現 | Pekko 互換ではなく、必要なら fraktor-native として扱う |

| 選択肢 | wire 互換 | 影響範囲 | 主なトレードオフ |
|--------|-----------|----------|------------------|
| A. wire-level compatibility | TCP framing で `AKKA` magic + stream id、Pekko protobuf control PDU、compression table の wire 表現まで互換 | `transport/tcp/frame_codec.rs`、`core/wire/*`、serializer manifest、compression 全体 | Pekko ノードとの相互運用が得られる。Rust 側 codec 設計の自由度を失う |
| B. responsibility parity のみ | 現行の fraktor 独自 framing / PDU を維持 | remote actor transport の責務分割だけ Pekko に寄せる | fraktor ノード同士の実装は単純。Pekko クラスタとは相互運用しない |

現行コードの `core/wire` は「Pekko Artery の責務分割を参考にした fraktor 独自 binary format」である。A を選ぶなら `Pekko Artery TCP framing`、`ArteryMessageSerializer`、`CompressionProtocol`、`DaemonMsgCreateSerializer` は byte compatibility 要件として実装する。B を選ぶなら、同じ責務は現行の fraktor wire format 上で実装する。

**決定 (2026-04-28): B. responsibility parity のみを採用する。** fraktor-rs は組み込み環境への展開余地を維持するため、`remote-core` の no_std + alloc 境界、軽量な独自 wire format、transport / runtime を adaptor 側へ分離する設計を優先する。Pekko ノードとの wire-level 相互運用は現時点の目標にしない。

この決定により、`Pekko Artery TCP framing`、`ArteryMessageSerializer` の protobuf control PDU、`DaemonMsgCreateSerializer` の byte compatibility、Artery compression table の wire 表現は実装優先度から外す。必要な責務は、既存の fraktor wire format 上で payload serialization、remote deployment、fraktor-native compression 設定 / protocol として扱う。

## 内部モジュール構造ギャップ

固定スコープ概念カバレッジは 85% だが、`hard` / `medium` ギャップが 11 件残っている。API / 実動作ギャップがまだ支配的なため、内部モジュール構造の詳細分析は後続フェーズとする。

| 構造観点 | 現状 | 次に見るべき点 |
|----------|------|----------------|
| `core::wire` と actor-core serialization の境界 | PDU はあるが `AnyMessage` payload を remote send path へ流す driver がない | `SerializationDelegator` を provider / transport / association runtime のどの層から呼ぶか |
| provider と actor-core provider の境界 | remote `ActorRef` materialization は実装済みだが、`send` は fail-fast | payload serialization と transport send の責務を provider 内に閉じるか、別 adapter に切り出すか |
| inbound delivery adapter | `WireFrame::Envelope` は受信できるが配送しない | local actor lookup、mailbox/system message queue、sender path 復元の責務分離 |
| watcher effect application | pure `WatcherState` と tokio actor はある | `NotifyTerminated` / `NotifyQuarantined` / `RewatchRemoteTargets` を actor-core に適用する adapter |
| flush 系契約 | `shutdown_flush_timeout` 設定だけ先行 | `FlushOnShutdown` / `FlushBeforeDeathWatchNotification` を core state と std driver に分けるか |

## 実装優先度

この節では、上で列挙したギャップだけを Phase に再配置する。

### Phase 1: trivial / easy

該当なし。公開 API surface を単純に足すだけで解消できる未実装ギャップは現時点ではない。

### Phase 2: medium

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| consistent-hashing pool remote router serialization（既定の `hash_key_mapper` を wire 表現できる場合に限定。任意クロージャの `hash_key_mapper` は wire 非対応） | actor-core/serialization | 5 |
| advanced Artery settings runtime application | core/config + std/runtime | 7 |

### Phase 3: hard

B 方針により、Pekko wire byte compatibility 固有の項目は Phase 3 から外す。Phase 3 は fraktor-rs 独自 wire 上で remote actor messaging を成立させるための hard gap に限定する。

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| inbound envelope delivery | std/association + actor-core | 3 |
| message payload serialization into envelope | std/provider + std/transport/tcp + actor-core/serialization | 4 |
| remote send path | std/provider + std/transport/tcp | 5 |
| remote DeathWatch interception | std/provider + std/watcher_actor + actor-core | 5 |
| remote deployment daemon / `useActorOnNode` | std/provider + actor-core | 5 |
| watcher effects application | std/watcher_actor + actor-core | 6 |
| `AddressTerminated` integration | actor-core + std/watcher_actor | 6 |
| `FlushOnShutdown` | std/extension_installer + std/association | 8 |
| `FlushBeforeDeathWatchNotification` | std/watcher_actor + std/association | 8 |

## まとめ

remote は address primitives、association state machine、failure detector + registry、typed `RemoteConfig`、TCP transport shell、inbound quarantine、restart budget、watcher UID protocol、resolve cache、remote `ActorRef` materialization、主要 misc serialization までカバー済みで、基礎部品の parity は進んでいる。

低コストで前進できる残タスクは Phase 2 の consistent-hashing pool remote router serialization（既定の `hash_key_mapper` を wire 表現できる場合に限定し、任意クロージャの `hash_key_mapper` は対象外）と advanced Artery settings runtime application である。Phase 1 の未実装ギャップは現時点ではない。

主要ギャップは Phase 3 の end-to-end remote actor delivery に集中している。payload serialization、remote `send`、inbound envelope delivery、remote DeathWatch / `AddressTerminated` 統合が揃うまでは、Pekko parity としての remote actor messaging は未完成である。
