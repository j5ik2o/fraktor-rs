# remote モジュール ギャップ分析

更新日: 2026-04-28 (8th edition / 現行ワークツリー再検証版)

## 比較スコープ定義

この分析は、Apache Pekko `remote` の raw API 全体を移植対象にするものではない。fraktor-rs の `remote` では、Pekko Artery compatible な remote actor transport 契約を parity 対象にし、classic remoting / JVM 実装技術 / testkit は分母から除外する。

スキル定義では `remote-core/src/domain/` を core 相当とする記述があるが、現行 `modules/remote-core/src/lib.rs` は `pub mod core;` を公開境界としており、`src/domain/` は存在しない。このため本レポートでは現行ツリーを優先し、`modules/remote-core/src/core/` を core 相当として扱う。

### 対象に含めるもの

| 領域 | fraktor-rs | Pekko 参照 |
|------|------------|------------|
| remote core | `modules/remote-core/src/core/` | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/` |
| Artery transport contract | `modules/remote-core/src/core/{transport,association,wire}/` | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/artery/` |
| std TCP adapter | `modules/remote-adaptor-std/src/std/{tcp_transport,association_runtime}/` | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/artery/tcp/` |
| remote actor ref provider | `modules/remote-core/src/core/provider/`, `modules/remote-adaptor-std/src/std/provider/` | `RemoteActorRefProvider.scala`, `RemoteActorRef` 相当 |
| failure detector / watcher | `modules/remote-core/src/core/{failure_detector,watcher}/`, `modules/remote-adaptor-std/src/std/watcher_actor/` | `FailureDetector*.scala`, `RemoteWatcher.scala` |
| serialization 接続点 | `modules/actor-core/src/core/kernel/serialization/`, `modules/remote-core/src/core/wire/` | `remote/serialization/` の remote transport に必要な契約 |
| lifecycle / instrumentation | `modules/remote-core/src/core/{extension,instrument,config}/`, `modules/remote-adaptor-std/src/std/extension_installer/` | `RemotingLifecycleEvent.scala`, `RemoteLogMarker.scala`, `RemoteInstrument.scala`, `RemoteSettings.scala`, `ArterySettings.scala` |

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
| Pekko 固定スコープ対象概念 | 77 |
| fraktor-rs 固定スコープ対応概念 | 60 |
| 固定スコープ概念カバレッジ | 60/77 (78%) |
| raw Pekko public type declarations | 360 |
| raw Pekko `def` declarations | 1594 |
| raw fraktor public type declarations | 83 (`remote-core`: 59, `remote-adaptor-std`: 24) |
| raw fraktor public method declarations | 338 (`remote-core`: 243, `remote-adaptor-std`: 95) |
| hard / medium / easy / trivial gap | 14 / 3 / 0 / 0 |

raw declaration count は private / deprecated / JVM 固有 API を含む参考値であり、parity 分母には使わない。

7th edition からの主な更新:

| 領域 | 現行評価 |
|------|----------|
| 公開境界 | 現行 `remote-core` は `src/core/` 公開であり、`src/domain/` は存在しないことを明記 |
| raw count | `remote-adaptor-std` の公開型・公開メソッド増加を反映 |
| `maximum_frame_size` | 受信 decode だけでなく outbound encode でも `WireFrameCodec` が上限超過を拒否するため、ギャップから除外 |
| queue / quarantine / restart 設定 | outbound queue size、remove quarantined association、outbound restart budget は実装済みとして維持 |
| 残ギャップ | remote `ActorRef` 実体化、payload serialization、inbound envelope delivery、remote DeathWatch 適用が引き続き支配的 |

`todo!()` / `unimplemented!()` / `panic!("not implemented")` は remote core / adaptor の production code から検出されない。一方で `Phase B minimum-viable` と placeholder コメントは残っており、主に remote `ActorRef` 実体化と payload serialization の未完成を示している。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core / remote primitives | address、unique address、association、wire PDU、failure detector、watcher state、provider contract、typed config | `modules/remote-core/src/core/` に整理済み。no_std 側の状態機械と PDU は揃っている | 公開 primitive は強い。残りは end-to-end runtime 接続 |
| std / adaptor | TCP listener/client、association runtime、remoting lifecycle、watcher actor、reconnect/backoff | `TcpRemoteTransport`、`AssociationRegistry`、`run_inbound_dispatch`、`run_outbound_loop_with_reconnect`、`WatcherActor` は存在 | bind / handshake / reconnect / quarantine filter は動く。message delivery は未完了 |
| actor-core integration | serialization registry、ActorRefProvider、DeathWatch、event stream、routing/deploy | serializer と local/loopback dispatch、cache event は接続済み | remote `ActorRef`、remote routee、DeathWatch 通知への統合が残る |

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
| inbound envelope delivery | `artery/MessageDispatcher.scala`, `artery/ArteryTcpTransport.scala` | 部分実装 | std/association_runtime + actor-core | hard | `run_inbound_dispatch` は `WireFrame::Envelope` を debug log するだけで local actor / mailbox へ配送しない |

### 4. Wire protocol / serialization　✅ 実装済み 11/17 (65%)

`FrameHeader`, `EnvelopePdu`, `HandshakePdu`, `ControlPdu`, `AckPdu` と各 codec、`MessageContainerSerializer`、`SystemMessageSerializer`、`MiscMessageSerializer` 主要 manifest、manifest-route fallback、outbound/inbound `maximum_frame_size` enforcement は実装済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| Pekko Artery TCP framing | `artery/tcp/TcpFraming.scala` | 未対応 | std/tcp_transport | hard | Pekko は `AKKA` magic + stream id + little-endian length。fraktor は独自 `length(4) + version(1) + kind(1)` |
| `ArteryMessageSerializer` control protocol | `serialization/ArteryMessageSerializer.scala`, `artery/Codecs.scala` | 部分実装 | core/wire + actor-core/serialization | hard | handshake / control / ack の責務はあるが、Pekko protobuf manifest / control message 互換ではない |
| message payload serialization into envelope | `MessageSerializer.scala`, `ArteryMessageSerializer.scala` | 部分実装 | std/tcp_transport + actor-core/serialization | hard | `TcpRemoteTransport::send` の `build_envelope_frame` が `Bytes::new()` placeholder を使う |
| `ActorIdentity` remote ActorRef restoration | `MiscMessageSerializer.scala` | 部分実装 | actor-core/serialization + std/provider | medium | `ActorIdentity` 自体は実装済みだが、remote path を remote `ActorRef` として復元する branch がない |
| `DaemonMsgCreateSerializer` | `serialization/DaemonMsgCreateSerializer.scala`, `RemoteDaemon.scala` | 未対応 | actor-core/serialization + std/provider | hard | remote deployment daemon と一体で必要 |
| Artery compression protocol | `artery/compress/*`, `artery/Codecs.scala` | 未対応 | core/wire + core/association | hard | actor ref / manifest compression table、advertisement、heavy hitter 検出がない |

### 5. Provider / remote actor ref / routing　✅ 実装済み 6/11 (55%)

`RemoteActorRef`, `RemoteActorRefProvider` trait、local/no-authority dispatch、loopback authority dispatch、`ActorRefResolveCache` 経由の remote resolve、cache hit/miss event publish は実装済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| concrete remote `ActorRef` construction | `RemoteActorRefProvider.scala`, `RemoteActorRef` | 部分実装 | std/provider | hard | remote branch は cache resolve 後に `RemoteSenderBuildFailed` を返す |
| remote send path | `RemoteActorRefProvider.scala`, `RemoteTransport.send` | 部分実装 | std/provider + std/tcp_transport | hard | `RemoteActorRefSender` は存在するが、現状は実 `ActorRef` に包まれず、payload も空 |
| remote DeathWatch interception | `RemoteActorRefProvider.scala`, `RemoteWatcher.scala` | 部分実装 | std/provider + std/watcher_actor + actor-core | hard | `watch` / `unwatch` intent は provider にあるが、actor-core DeathWatch への最終接続がない |
| `RemoteRouterConfig` runtime routee expansion / remaining pool variants | `remote/routing/RemoteRouterConfig.scala` | 部分実装 | actor-core/routing + std/provider | medium | `RemoteRouterConfig<SmallestMailboxPool>`, `<RoundRobinPool>`, `<RandomPool>` と serializer はある。remote node list に routee を実体化する経路と追加 pool は未完了 |
| remote deployment daemon / `useActorOnNode` | `RemoteDaemon.scala`, `RemoteDeployer.scala`, `RemoteDeploymentWatcher.scala` | 未対応 | std/provider + actor-core | hard | remote child actor 作成要求と deployment watcher がない |

### 6. Watcher / DeathWatch runtime　✅ 実装済み 5/7 (71%)

`WatcherState`, `WatcherCommand`, `WatcherEffect`, `WatcherActor`, `run_heartbeat_loop`, heartbeat response UID tracking、UID 変更時の rewatch effect は実装済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| watcher effects application | `RemoteWatcher.scala` | 部分実装 | std/watcher_actor + actor-core | hard | `WatcherActor` は effects を `effect_tx` へ流すだけで、`Terminated` / event stream / system message に適用しない |
| `AddressTerminated` integration | `RemoteWatcher.scala`, actor event stream | 未対応 | actor-core + std/watcher_actor | hard | remote node failure を local DeathWatch へ統合する契約がない |

### 7. Instrumentation / config / logging　✅ 実装済み 8/9 (89%)

`RemotingLifecycleState`, `StdRemoting`, `EventPublisher`, `RemoteLogMarker`, `RemoteInstrument`, `RemotingFlightRecorder`, `RemoteAuthoritySnapshot`、主要 `RemoteConfig` builder は実装済み。`bind_hostname` / `bind_port` / `inbound_lanes` / `outbound_lanes` / `maximum_frame_size` / `buffer_pool_size` / `untrusted_mode` / log toggle / outbound queue / remove-quarantined / outbound restart budget は現行コードで確認済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| advanced Artery settings 残り | `ArterySettings.scala` | 部分実装 | core/config + std/runtime | medium | 未導入は `outbound-large-message-queue-size`、`large-message-destinations`、`inbound-restart-timeout`、`inbound-max-restarts`、compression settings など。Aeron/TLS/HOCON 固有は n/a |

### 8. Reliability / lifecycle adapter　✅ 実装済み 2/4 (50%)

`InboundQuarantineCheck` と `RestartCounter` は実装済み。shutdown / DeathWatch 前 flush は残る。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| `FlushOnShutdown` | `artery/FlushOnShutdown.scala` | 未対応 | std/extension_installer + std/association_runtime | hard | `shutdown_flush_timeout` 設定はあるが、association に termination hint を送り ack を待つ driver がない |
| `FlushBeforeDeathWatchNotification` | `artery/FlushBeforeDeathWatchNotification.scala` | 未対応 | std/watcher_actor + std/association_runtime | hard | DeathWatch 通知前に対象 association を flush する契約がない |

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
| `artery/jfr/Events.scala`, `JFRRemotingFlightRecorder.scala` | JVM Flight Recorder 固有。Rust 側は `RemotingFlightRecorder` で代替 |
| HOCON provider loading / `FailureDetectorLoader` 動的ロード / JVM classloader | JVM 設定ロード方式 |
| `TestStage`, multi-node-testkit, remote-tests | runtime API ではない |

## 方針判断

Phase 3 hard の実装は、次の方針判断に強く依存する。

### Q. Pekko Artery と wire-protocol parity を目指すか

| 選択肢 | wire 互換 | 影響範囲 | 主なトレードオフ |
|--------|-----------|----------|------------------|
| A. protocol parity | TCP framing で `AKKA` magic + stream id、Pekko protobuf control PDU、compression table の wire 表現まで互換 | `tcp_transport/frame_codec.rs`、`core/wire/*`、serializer manifest、compression 全体 | Pekko ノードとの相互運用が得られる。Rust 側 codec 設計の自由度を失う |
| B. responsibility parity のみ | 現状の独自 framing / PDU を維持 | remote actor transport の責務分割だけ Pekko に寄せる | fraktor ノード同士の実装は単純。Pekko クラスタとは相互運用しない |

現行コードの `core/wire` は「Pekko Artery の責務分割を参考にした独自 binary format」に近い。A を選ぶなら `Pekko Artery TCP framing`、`ArteryMessageSerializer`、`CompressionProtocol`、`DaemonMsgCreateSerializer` は byte compatibility 要件として実装する。B を選ぶなら同じ項目は「Pekko と同じ責務を独自 wire で閉じる」実装タスクになる。

## 内部モジュール構造ギャップ

固定スコープ概念カバレッジは 78% で、`hard` / `medium` ギャップが 17 件残っている。API / 実動作ギャップがまだ支配的なため、内部モジュール構造の詳細分析は後続フェーズとする。

| 構造観点 | 現状 | 次に見るべき点 |
|----------|------|----------------|
| `core::wire` と actor-core serialization の境界 | PDU はあるが `AnyMessage` payload が空 bytes | `SerializationDelegator` をどの層から呼ぶか |
| provider と actor-core provider の境界 | local / loopback dispatch はあるが remote branch は `RemoteSenderBuildFailed` | `ActorSystemState` 依存を provider / extension installer のどちらに閉じるか |
| inbound delivery adapter | `WireFrame::Envelope` は受信できるが配送しない | local actor lookup、mailbox/system message queue、sender path 復元の責務分離 |
| watcher effect application | pure `WatcherState` と tokio actor はある | `NotifyTerminated` / `NotifyQuarantined` / `RewatchRemoteTargets` を actor-core に適用する adapter |
| flush 系契約 | `shutdown_flush_timeout` 設定だけ先行 | `FlushOnShutdown` / `FlushBeforeDeathWatchNotification` を core state と std driver に分けるか |

## 実装優先度

この節では、上で列挙したギャップだけを Phase に再配置する。

### Phase 1: trivial / easy

該当なし。7th edition で easy 扱いだった outbound `maximum_frame_size` enforcement は現行コードで実装済み。

### Phase 2: medium

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `ActorIdentity` remote ActorRef restoration | actor-core/serialization + std/provider | 4 |
| `RemoteRouterConfig` runtime routee expansion / remaining pool variants | actor-core/routing + std/provider | 5 |
| advanced Artery settings 残り | core/config + std/runtime | 7 |

### Phase 3: hard

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| inbound envelope delivery | std/association_runtime + actor-core | 3 |
| Pekko Artery TCP framing | std/tcp_transport | 4 |
| `ArteryMessageSerializer` control protocol | core/wire + actor-core/serialization | 4 |
| message payload serialization into envelope | std/tcp_transport + actor-core/serialization | 4 |
| `DaemonMsgCreateSerializer` | actor-core/serialization + std/provider | 4 |
| Artery compression protocol | core/wire + core/association | 4 |
| concrete remote `ActorRef` construction | std/provider | 5 |
| remote send path | std/provider + std/tcp_transport | 5 |
| remote DeathWatch interception | std/provider + std/watcher_actor + actor-core | 5 |
| remote deployment daemon / `useActorOnNode` | std/provider + actor-core | 5 |
| watcher effects application | std/watcher_actor + actor-core | 6 |
| `AddressTerminated` integration | actor-core + std/watcher_actor | 6 |
| `FlushOnShutdown` | std/extension_installer + std/association_runtime | 8 |
| `FlushBeforeDeathWatchNotification` | std/watcher_actor + std/association_runtime | 8 |

## まとめ

remote は address primitives、association state machine、failure detector + registry、typed `RemoteConfig`、TCP transport shell、inbound quarantine、restart budget、watcher UID protocol、resolve cache、主要 misc serialization までカバー済みで、基礎部品の parity は進んでいる。

低コストで前進できる残タスクは Phase 2 の `ActorIdentity` remote ActorRef restoration、`RemoteRouterConfig` runtime routee expansion、advanced Artery settings 残りである。Phase 1 の未実装ギャップは現時点ではない。

主要ギャップは Phase 3 の end-to-end remote actor delivery に集中している。`RemoteActorRef` 実体化、payload serialization、inbound envelope delivery、remote DeathWatch / `AddressTerminated` 統合が揃うまでは、Pekko parity としての remote actor messaging は未完成である。
