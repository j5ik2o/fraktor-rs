# remote モジュール ギャップ分析

更新日: 2026-05-15 (16th edition / deployment response close 分類後の再検証)

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

remote は address primitives、failure detector、association state、wire PDU、TCP transport shell、compression table application、resolve cache、remote `ActorRef` materialization、actor-core serialization registry backed payload の outbound / inbound delivery、remote deployment create、deployment response timeout / closed channel 分類まで実装済みである。固定スコープ概念カバレッジは 74/75 (98.7%) である。

一方で、残ギャップは `AddressTerminated` integration に集中している。remote DeathWatch の watch / unwatch / notification delivery、ACK/NACK redelivery、shutdown / DeathWatch 前 flush lifecycle、RemoteScope child create は接続済みである。

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 75 |
| fraktor-rs 固定スコープ対応概念 | 74 |
| 固定スコープ概念カバレッジ | 74/75 (98.7%) |
| raw Pekko public type declarations | 361（Scala / Java、protobuf 除外） |
| raw Pekko `def` declarations | 1594 |
| raw fraktor public type declarations | 115（`remote-core`: 85 / `remote-adaptor-std`: 30、production rs 再計測） |
| raw fraktor public method declarations | 488（`remote-core`: 406 / `remote-adaptor-std`: 82、production rs 再計測） |
| hard / medium / easy / trivial gap | 1 / 0 / 0 / 0 |

`todo!()` / `unimplemented!()` / `panic!("not implemented")` と production code 上の明示 TODO は remote core / adaptor から検出されない。test helper 上の stub コメントは parity gap から除外する。`modules/remote-core/src/wire/primitives.rs:12` の header placeholder は encode 時の長さ埋め戻しであり、未実装ギャップには分類しない。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core | address、unique address、association、wire PDU、compression table、failure detector、watcher state、provider contract、typed config、deployment PDU | `modules/remote-core/src/` に整理済み。no_std 側の状態機械、PDU、compression table は揃っている | 公開 primitive は強い。`AddressTerminated` integration は残る |
| std / adaptor | TCP listener/client、association 実行系、remoting lifecycle、inbound dispatch、reconnect/backoff、serialized payload delivery、compression table application、watcher task | `TcpRemoteTransport`、`run_inbound_dispatch`、`run_remote_with_delivery`、`StdRemoteActorRefProvider`、watcher task は存在 | bind / handshake / reconnect / quarantine filter / actor-core serializer backed payload delivery / compression advertisement/ack / remote DeathWatch / flush driver は動く |
| actor-core integration | serialization registry、ActorRefProvider、DeathWatch、event stream、routing/deploy | misc serializer、scheme provider lookup、remote `ActorRef` materialization、routee expansion、registered payload remote send、remote DeathWatch 通知、RemoteScope child create は接続済み | `AddressTerminated` integration が残る |

## カテゴリ別ギャップ

ギャップ表には未対応・部分実装・n/a のみを列挙する。実装済み項目はカテゴリ件数に含めるが、表には出さない。

### 1. Address / identity ✅ 実装済み 4/4 (100%)

`Address`, `UniqueAddress`, `RemoteNodeId`, `resolve_remote_address` は実装済み。Pekko の `UniqueAddress(address, uid)` と同じ責務を持つ。

### 2. Failure detector ✅ 実装済み 6/6 (100%)

`FailureDetector`, `DeadlineFailureDetector`, `PhiAccrualFailureDetector`, `HeartbeatHistory`, `FailureDetectorRegistry`, `DefaultFailureDetectorRegistry` は実装済み。address-bound detector registry も no_std core に入っている。

### 3. Transport / association / lifecycle ✅ 実装済み 18/18 (100%)

`Association`, `AssociationEffect`, `SendQueue`, `QuarantineReason`, `HandshakeValidationError`, `RemoteTransport`, `TcpRemoteTransport`, handshake timeout、connection lost recovery、inbound quarantine、restart/backoff、large-message queue selection、inbound / outbound TCP lanes、serialized payload の outbound / inbound delivery、ACK/NACK redelivery state application は実装済み。

### 4. Wire protocol / serialization ✅ 実装済み 14/14 (100%)

`FrameHeader`, `EnvelopePdu`, `HandshakePdu`, `ControlPdu`, `AckPdu` と各 codec、serializer id / manifest / payload bytes を持つ envelope layout、actor-ref / manifest 用 `CompressedText` metadata、compression advertisement / ack control PDU、manifest-route fallback を持つ actor-core serialization registry、`ActorIdentity` / `RemoteScope` / remote router config の misc serialization、outbound / inbound `maximum_frame_size` enforcement、`Vec<u8>` / `ByteString` / `String` など登録済み payload の outbound serialize / inbound deserialize は実装済み。`bytes::Bytes` は builtin serializer 対象ではないため、custom serializer 未登録では拒否する。

### 5. Provider / remote actor ref / routing ✅ 実装済み 11/11 (100%)

`RemoteActorRef`, `RemoteActorRefProvider` trait、local/no-authority dispatch、loopback authority dispatch、`ActorRefResolveCache` 経由の remote resolve、cache hit/miss event publish、concrete remote `ActorRef` construction、registered payload remote send、remote router config serialization、remote DeathWatch hook interception、RemoteScope child create request/response、deployment response dispatcher、bounded stale response handling、timeout / closed-channel classification は実装済み。

### 6. Watcher / DeathWatch 実行系 ✅ 実装済み 6/7 (86%)

`WatcherState`, `WatcherCommand`, `WatcherEffect`, heartbeat response UID tracking、UID 変更時の `RewatchRemoteTargets` effect、std watcher task、watch / unwatch / rewatch / notification effect application は実装済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| `AddressTerminated` integration | `RemoteWatcher.scala:26`, `RemoteWatcher.scala:95`, `RemoteWatcher.scala:205` | 未対応 | actor-core-kernel + std | hard | remote node failure を actor-core event stream の address-terminated topic 相当へ統合する契約がない |

### 7. Instrumentation / config / logging ✅ 実装済み 9/9 (100%)

`RemotingLifecycleState`, `Remote`, `RemoteShared`, `EventPublisher`, `RemoteLogMarker`, `RemoteInstrument`, `RemotingFlightRecorder`, `RemoteAuthoritySnapshot`、主要 `RemoteConfig` builder は実装済み。`bind_hostname` / `bind_port` / `inbound_lanes` / `outbound_lanes` / `maximum_frame_size` / `buffer_pool_size` / `untrusted_mode` / log toggle / outbound queue / remove-quarantined / outbound restart budget / inbound restart budget / large-message destinations / compression config は現行コードで確認済み。

### 8. Reliability / lifecycle adaptor ✅ 実装済み 4/4 (100%)

`InboundQuarantineCheck` 相当の quarantine handling、connection loss recovery、`FlushOnShutdown` 相当の shutdown flush wait、`FlushBeforeDeathWatchNotification` 相当の DeathWatch notification 前 flush gate、remote deployment response channel close の timeout からの分類は実装済み。

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

固定スコープ概念カバレッジは 98.7% で 80% を超えるため、公開 API の残ギャップを実装する上での構造差分も記録する。残る API gap はすべて `hard` であり、次の構造ギャップが実装順序を制約している。

| 構造ギャップ | Pekko側の根拠 | fraktor-rs側の現状 | 推奨アクション | 難易度 | 緊急度 | 備考 |
|-------------|---------------|--------------------|----------------|--------|--------|------|
| `AddressTerminated` integration 境界不足 | `RemoteWatcher.scala:26`, `RemoteWatcher.scala:95`, `RemoteWatcher.scala:205` | watcher state と DeathWatch 通知はあるが、remote node failure を actor-core event stream の address-terminated topic 相当へ統合する契約がない | watcher failure outcome を actor-core event stream / lifecycle notification へ接続 | hard | medium | node-level failure と actor-level termination の境界整理が必要 |

## 実装優先度

この節では、上で列挙したギャップだけを Phase に再配置する。YAGNI は適用せず、Pekko parity ギャップを埋める順序として扱う。

### Phase 1: trivial / easy

該当なし。公開 API surface を単純に足すだけで解消できる未実装ギャップは現時点ではない。

### Phase 2: medium

該当なし。残ギャップはいずれも serialization / transport / actor-core integration / lifecycle driver にまたがる。

### Phase 3: hard

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `AddressTerminated` integration | actor-core-kernel + std | 6 |

## まとめ

remote は address primitives、association state machine、ACK/NACK redelivery、failure detector + registry、typed `RemoteConfig`、TCP transport shell、inbound quarantine、restart handling、compression table application、resolve cache、remote `ActorRef` materialization、remote DeathWatch、shutdown / DeathWatch 前 flush、主要 misc serialization、registered payload の二ノード配送、remote deployment create、deployment response timeout / closed channel 分類までカバー済みで、基礎部品の parity は進んでいる。

Phase 1 / Phase 2 の低コストギャップは現時点ではない。parity を次に進めるには、Phase 3 の `AddressTerminated` integration を詰める必要がある。

主要ギャップは `AddressTerminated` integration に集中している。API カバレッジは 90% を超えているため、次のボトルネックは単なる型追加ではなく、remote node failure と actor-core notification の接続境界にある。
