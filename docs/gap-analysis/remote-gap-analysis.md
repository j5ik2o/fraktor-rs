# remote モジュール ギャップ分析

更新日: 2026-05-14 (13th edition / payload serialization 接続後)

## 比較スコープ定義

この分析は Apache Pekko `remote` の raw API 全体を移植対象にするものではない。fraktor-rs の `remote` では、Pekko Artery compatible な remote actor transport 契約を parity 対象にし、classic remoting、JVM 実装技術、testkit、Pekko wire byte compatibility は分母から除外する。

スコープ定義では `modules/remote-core/src/domain/` を core 相当とする記述があるが、現行 `modules/remote-core/src/lib.rs` は crate root から remote core modules を公開しており、`src/domain/` は存在しない。このため本レポートでは現行ツリーを優先し、`modules/remote-core/src/` を core 相当として扱う。

### 対象に含めるもの

| 領域 | fraktor-rs | Pekko 参照 |
|------|------------|------------|
| remote core | `modules/remote-core/src/` | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/` |
| Artery transport contract | `modules/remote-core/src/{association,envelope,transport,wire}/` | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/artery/` |
| std TCP adaptor | `modules/remote-adaptor-std/src/{association,extension_installer,transport/tcp,watcher}/` | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/artery/tcp/` |
| remote actor ref provider | `modules/remote-core/src/provider/`, `modules/remote-adaptor-std/src/provider/` | `RemoteActorRefProvider.scala`, `RemoteActorRef` 相当 |
| failure detector / watcher | `modules/remote-core/src/{failure_detector,watcher}/` | `FailureDetector*.scala`, `RemoteWatcher.scala` |
| serialization 接続点 | `modules/actor-core-kernel/src/serialization/`, `modules/remote-core/src/wire/` | `remote/serialization/`, `artery/Codecs.scala` の transport 契約部分 |
| lifecycle / instrumentation | `modules/remote-core/src/{extension,instrument,config}/`, `modules/remote-adaptor-std/src/extension_installer/` | `RemotingLifecycleEvent.scala`, `RemoteLogMarker.scala`, `RemoteInstrument.scala`, `RemoteSettings.scala`, `ArterySettings.scala` |

### 対象から除外するもの

| 除外項目 | 理由 |
|----------|------|
| classic remoting / `Endpoint.scala` / `AckedDelivery.scala` | Pekko 側でも deprecated。Artery 互換の分母には入れない |
| `transport/netty/`, `PekkoProtocolTransport.scala`, `PekkoPduCodec.scala`, `AbstractTransportAdapter.scala` | classic transport stack |
| Aeron UDP transport (`artery/aeron/*`) | JVM Aeron 固有実装。Rust std TCP adaptor とは別物 |
| TLS / `SSLEngineProvider` / `security/provider/*` | Java `SSLEngine` / HOCON / classloader に依存する完全互換は除外。Rust TLS adaptor が必要なら別スコープ |
| Java serialization / Jackson module 完全互換 | serialization contract との接続点だけ対象 |
| HOCON provider loading / `FailureDetectorLoader` 動的ロード / JVM classloader | JVM 設定ロード方式に依存 |
| JFR `artery/jfr/Events.scala`, `JFRRemotingFlightRecorder.scala` | JVM 固有。Rust 側は `RemotingFlightRecorder` で代替 |
| remote testkit / multi-node-testkit / remote-tests | 実行時 API ではない |
| `RemoteMetricsExtension`, `AddressUidExtension`, `BoundAddressesExtension` | JVM 拡張ローダ依存。同等情報は `RemotingLifecycleState` / `RemoteAuthoritySnapshot` で再現する |
| `EnvelopeBufferPool`, `ObjectPool`, `FixedSizePartitionHub` | JVM GC 回避目的の最適化用 buffer pool。Rust では割り当て戦略が異なる |
| `ImmutableLongMap`, `LruBoundedCache` | internal collection helper。Rust では `hashbrown` / `BTreeMap` / 専用 cache で代替 |
| Pekko Artery TCP framing byte compatibility | fraktor は独自 `length(4) + version(1) + kind(1)` framing を維持する |
| Pekko protobuf control PDU byte compatibility | responsibility parity だけを対象にし、Pekko ノードとの wire-level 相互運用は目標にしない |

raw declaration count は Scala / Java / JVM 固有 API を含む参考値であり、parity 分母には使わない。固定スコープでは、Rust の no_std / std 境界上で再現可能な remote actor transport 契約だけを分母にする。

## サマリー

remote は address primitives、failure detector、association state、wire PDU、TCP transport shell、resolve cache、remote `ActorRef` materialization、actor-core serialization registry backed payload の outbound / inbound delivery まで実装済みである。

一方で、残ギャップは compression table application、remote deployment、`AddressTerminated` integration、flush lifecycle に集中している。remote DeathWatch の watch / unwatch / notification delivery と ACK/NACK redelivery は `remote-reliable-deathwatch` で接続済みである。

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 75 |
| fraktor-rs 固定スコープ対応概念 | 70 |
| 固定スコープ概念カバレッジ | 70/75 (93.3%) |
| raw Pekko public type declarations | 361（Scala / Java、protobuf 除外） |
| raw Pekko `def` declarations | 1594 |
| raw fraktor public type declarations | 86（`remote-core`: 70 / `remote-adaptor-std`: 16） |
| raw fraktor public method declarations | 352（`remote-core`: 312 / `remote-adaptor-std`: 40） |
| hard / medium / easy / trivial gap | 5 / 0 / 0 / 0 |

`todo!()` / `unimplemented!()` / `panic!("not implemented")` と production code 上の明示 TODO は remote core / adaptor から検出されない。`modules/remote-core/src/wire/primitives.rs:12` の header placeholder は encode 時の長さ埋め戻しであり、未実装ギャップには分類しない。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core | address、unique address、association、wire PDU、failure detector、watcher state、provider contract、typed config | `modules/remote-core/src/` に整理済み。no_std 側の状態機械と PDU は揃っている | 公開 primitive は強い。compression / deployment / flush は残る |
| std / adaptor | TCP listener/client、association 実行系、remoting lifecycle、inbound dispatch、reconnect/backoff、serialized payload delivery、watcher task | `TcpRemoteTransport`、`run_inbound_dispatch`、`run_remote_with_delivery`、`StdRemoteActorRefProvider`、watcher task は存在 | bind / handshake / reconnect / quarantine filter / actor-core serializer backed payload delivery / remote DeathWatch は動く。flush driver はない |
| actor-core integration | serialization registry、ActorRefProvider、DeathWatch、event stream、routing/deploy | misc serializer、scheme provider lookup、remote `ActorRef` materialization、routee expansion、registered payload remote send、remote DeathWatch 通知は接続済み | remote deployment と `AddressTerminated` integration が残る |

## カテゴリ別ギャップ

ギャップ表には未対応・部分実装・n/a のみを列挙する。実装済み項目はカテゴリ件数に含めるが、表には出さない。

### 1. Address / identity ✅ 実装済み 4/4 (100%)

`Address`, `UniqueAddress`, `RemoteNodeId`, `resolve_remote_address` は実装済み。Pekko の `UniqueAddress(address, uid)` と同じ責務を持つ。

### 2. Failure detector ✅ 実装済み 6/6 (100%)

`FailureDetector`, `DeadlineFailureDetector`, `PhiAccrualFailureDetector`, `HeartbeatHistory`, `FailureDetectorRegistry`, `DefaultFailureDetectorRegistry` は実装済み。address-bound detector registry も no_std core に入っている。

### 3. Transport / association / lifecycle ✅ 実装済み 18/18 (100%)

`Association`, `AssociationEffect`, `SendQueue`, `QuarantineReason`, `HandshakeValidationError`, `RemoteTransport`, `TcpRemoteTransport`, handshake timeout、connection lost recovery、inbound quarantine、restart/backoff、large-message queue selection、inbound / outbound TCP lanes、serialized payload の outbound / inbound delivery、ACK/NACK redelivery state application は実装済み。

### 4. Wire protocol / serialization ✅ 実装済み 13/14 (93%)

`FrameHeader`, `EnvelopePdu`, `HandshakePdu`, `ControlPdu`, `AckPdu` と各 codec、serializer id / manifest / payload bytes を持つ envelope layout、manifest-route fallback を持つ actor-core serialization registry、`ActorIdentity` / `RemoteScope` / remote router config の misc serialization、outbound / inbound `maximum_frame_size` enforcement、`Vec<u8>` / `ByteString` / `String` など登録済み payload の outbound serialize / inbound deserialize は実装済み。`bytes::Bytes` は builtin serializer 対象ではないため、custom serializer 未登録では拒否する。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| actor-ref / manifest compression advertisement and table application | `artery/compress/CompressionProtocol.scala:22`, `artery/compress/InboundCompressions.scala:39`, `artery/Codecs.scala:260`, `artery/ArteryTransport.scala:504` | 部分実装 | core/wire + std/transport/tcp + actor-core-kernel/serialization | hard | `RemoteCompressionConfig` はあるが、compression table、advertisement / ack control message、actor ref / manifest hit counting、encoder / decoder への table application は未配置。Pekko byte compatibility ではなく fraktor-native wire 上の responsibility parity として扱う |

### 5. Provider / remote actor ref / routing ✅ 実装済み 10/11 (91%)

`RemoteActorRef`, `RemoteActorRefProvider` trait、local/no-authority dispatch、loopback authority dispatch、`ActorRefResolveCache` 経由の remote resolve、cache hit/miss event publish、concrete remote `ActorRef` construction、registered payload remote send、remote router config serialization、remote DeathWatch hook interception は実装済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| remote deployment daemon / `useActorOnNode` | `RemoteDaemon.scala:59`, `RemoteActorRefProvider.scala:596`, `RemoteDeploymentWatcher.scala:37`, `DaemonMsgCreateSerializer.scala:40` | 未対応 | std/provider + actor-core-kernel | hard | `RemoteScope` と `RemoteRouterConfig` は actor-core にあるが、remote child actor 作成要求、daemon message、deployment watcher、allow-list / untrusted-mode guard はない |

### 6. Watcher / DeathWatch 実行系 ✅ 実装済み 6/7 (86%)

`WatcherState`, `WatcherCommand`, `WatcherEffect`, heartbeat response UID tracking、UID 変更時の `RewatchRemoteTargets` effect、std watcher task、watch / unwatch / rewatch / notification effect application は実装済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| `AddressTerminated` integration | `RemoteWatcher.scala:26`, `RemoteWatcher.scala:95`, `RemoteWatcher.scala:205` | 未対応 | actor-core-kernel + std | hard | remote node failure を actor-core event stream の address-terminated topic 相当へ統合する契約がない |

### 7. Instrumentation / config / logging ✅ 実装済み 9/9 (100%)

`RemotingLifecycleState`, `Remote`, `RemoteShared`, `EventPublisher`, `RemoteLogMarker`, `RemoteInstrument`, `RemotingFlightRecorder`, `RemoteAuthoritySnapshot`、主要 `RemoteConfig` builder は実装済み。`bind_hostname` / `bind_port` / `inbound_lanes` / `outbound_lanes` / `maximum_frame_size` / `buffer_pool_size` / `untrusted_mode` / log toggle / outbound queue / remove-quarantined / outbound restart budget / inbound restart budget / large-message destinations / compression config は現行コードで確認済み。

### 8. Reliability / lifecycle adaptor ✅ 実装済み 2/4 (50%)

`InboundQuarantineCheck` 相当の quarantine handling と connection loss recovery は実装済み。shutdown / DeathWatch 前 flush は残る。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| `FlushOnShutdown` | `artery/FlushOnShutdown.scala:30`, `artery/FlushOnShutdown.scala:52`, `artery/FlushOnShutdown.scala:79` | 未対応 | std/extension_installer + std/association | hard | `shutdown_flush_timeout` 設定はあるが、association に termination hint / flush frame を送り ack を待つ driver がない |
| `FlushBeforeDeathWatchNotification` | `artery/FlushBeforeDeathWatchNotification.scala:33`, `artery/FlushBeforeDeathWatchNotification.scala:65`, `artery/FlushBeforeDeathWatchNotification.scala:85` | 未対応 | std/watcher + std/association | hard | DeathWatch 通知前に対象 association を flush する契約がない |

### 9. Internal helpers / cache ✅ 実装済み 2/2 (100%)

`ActorRefResolveCache` と `RemoteActorRefResolveCacheEvent` / `RemoteActorRefResolveCacheOutcome` は実装済み。`StdRemoteActorRefProvider` から hit/miss event も publish される。

## 対象外 n/a

| Pekko API / 領域 | 判定理由 |
|------------------|----------|
| classic remoting `Endpoint*`, `AckedDelivery`, `PekkoProtocolTransport`, `PekkoPduCodec`, `transport/Transport.scala` | deprecated classic remoting |
| `transport/netty/*`, `FailureInjectorTransportAdapter`, `ThrottlerTransportAdapter`, `TestTransport` | classic transport / fault injection / test 用 |
| Aeron UDP transport (`artery/aeron/{ArteryAeronUdpTransport,AeronSink,AeronSource,TaskRunner}`) | JVM Aeron 固有 |
| `SSLEngineProvider`, `ConfigSSLEngineProvider`, `RotatingKeysSSLEngineProvider`, `security/provider/*` | Java `SSLEngine` 完全互換は対象外。Rust TLS adaptor が必要なら別スコープ |
| Java serialization / Jackson module 完全互換 | serializer contract との接続点だけ対象 |
| `RemoteMetricsExtension`, `AddressUidExtension`, `BoundAddressesExtension` | JVM 拡張ローダ依存。同等情報は `RemotingLifecycleState` / `RemoteAuthoritySnapshot` で再現 |
| `EnvelopeBufferPool`, `ObjectPool`, `FixedSizePartitionHub` | JVM GC 回避用 buffer pool |
| `ImmutableLongMap`, `LruBoundedCache` | internal collection helper |
| `ProtobufSerializer` | Pekko 内部の protobuf bridge。fraktor は独自 binary codec |
| Pekko Artery TCP framing byte compatibility | fraktor は独自 framing を維持する |
| `ArteryMessageSerializer` protobuf control protocol byte compatibility | responsibility parity のみ対象。handshake / control / ack の責務は fraktor 独自 PDU で閉じる |
| `DaemonMsgCreateSerializer` byte compatibility | remote deployment 責務が必要な場合も fraktor 独自 serializer として扱う |
| Artery compression table wire compatibility | compression が必要な場合も fraktor 独自 wire 上の責務として扱う |
| `artery/jfr/Events.scala`, `JFRRemotingFlightRecorder.scala` | JVM Flight Recorder 固有。Rust 側は `RemotingFlightRecorder` で代替 |
| HOCON provider loading / `FailureDetectorLoader` 動的ロード / JVM classloader | JVM 設定ロード方式 |
| `TestStage`, multi-node-testkit, remote-tests | 実行時 API ではない |

## 内部モジュール構造ギャップ

固定スコープ概念カバレッジは 93.3% で 80% を超えるため、公開 API の残ギャップを実装する上での構造差分も記録する。残る API gap はすべて `hard` であり、次の構造ギャップが実装順序を制約している。

| 構造ギャップ | Pekko側の根拠 | fraktor-rs側の現状 | 推奨アクション | 難易度 | 緊急度 | 備考 |
|-------------|---------------|--------------------|----------------|--------|--------|------|
| compression table 所有者不足 | `artery/compress/InboundCompressions.scala:39`, `artery/Codecs.scala:260` | `RemoteCompressionConfig` は設定値のみで、table state / advertisement owner がない | core に table state、std に advertisement timer / control delivery を配置 | hard | medium | serializer registry 接続後に実装しやすい |
| flush protocol の PDU / driver 不足 | `FlushOnShutdown.scala:52`, `FlushBeforeDeathWatchNotification.scala:65` | `shutdown_flush_timeout` はあるが、flush frame / ack / wait driver がない | core wire に flush control、std association に wait driver を追加 | hard | medium | DeathWatch 前 flush と shutdown flush の共通基盤 |
| remote deployment の責務境界不足 | `RemoteActorRefProvider.scala:596`, `RemoteDaemon.scala:59` | actor-core に `RemoteScope` はあるが、remote daemon / deployment watcher がない | std provider と actor-core deployer の境界に remote create command を追加 | hard | medium | deployer と serialization の両方にまたがる |

## 実装優先度

この節では、上で列挙したギャップだけを Phase に再配置する。YAGNI は適用せず、Pekko parity ギャップを埋める順序として扱う。

### Phase 1: trivial / easy

該当なし。公開 API surface を単純に足すだけで解消できる未実装ギャップは現時点ではない。

### Phase 2: medium

該当なし。残ギャップはいずれも serialization / transport / actor-core integration / lifecycle driver にまたがる。

### Phase 3: hard

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| actor-ref / manifest compression advertisement and table application | core/wire + std/transport/tcp + actor-core-kernel/serialization | 4 |
| remote deployment daemon / `useActorOnNode` | std/provider + actor-core-kernel | 5 |
| `AddressTerminated` integration | actor-core-kernel + std | 6 |
| `FlushOnShutdown` | std/extension_installer + std/association | 8 |
| `FlushBeforeDeathWatchNotification` | std/watcher + std/association | 8 |

## まとめ

remote は address primitives、association state machine、ACK/NACK redelivery、failure detector + registry、typed `RemoteConfig`、TCP transport shell、inbound quarantine、restart handling、resolve cache、remote `ActorRef` materialization、remote DeathWatch、主要 misc serialization、registered payload の二ノード配送までカバー済みで、基礎部品の parity は進んでいる。

Phase 1 / Phase 2 の低コストギャップは現時点ではない。parity を次に進めるには、Phase 3 のうち actor-ref / manifest compression table application を先に通すのが効果的である。

主要ギャップは compression table application、`AddressTerminated` integration、remote deployment、flush lifecycle に集中している。API カバレッジは 90% を超えているため、次のボトルネックは単なる型追加ではなく、compression table / deployment / flush の接続境界にある。
