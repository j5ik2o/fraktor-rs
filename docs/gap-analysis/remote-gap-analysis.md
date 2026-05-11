# remote モジュール ギャップ分析

更新日: 2026-05-11 (11th edition / Phase 2 medium gaps 反映版)

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
| serialization 接続点 | `modules/actor-core-kernel/src/serialization/`, `modules/remote-core/src/wire/` | `remote/serialization/` の remote transport に必要な契約 |
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

### 判定方針

`references/pekko` から機械的に抽出した raw 公開型数・メソッド数は参考値に留める。Scala / Java / JVM 固有 API を分母に入れると Rust では再現不能なギャップが混ざるため、このレポートでは上記固定スコープだけを parity 分母にする。

## サマリー

remote は address primitives、failure detector、association state、wire PDU、TCP transport shell、resolve cache、remote `ActorRef` materialization まで実装済みである。前回の主な未完成点だった byte payload の outbound send と inbound delivery に加え、Phase 2 medium gap だった wire-safe consistent-hashing remote router serialization、large-message outbound queue selection、inbound / outbound TCP lanes の設定適用も実装済みである。

残ギャップは、Pekko の serializer registry による任意 actor message serialization、ACK/NACK に基づく redelivery、remote DeathWatch、remote deployment、flush lifecycle に集中している。

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 74 |
| fraktor-rs 固定スコープ対応概念 | 66 |
| 固定スコープ概念カバレッジ | 66/74 (89.2%) |
| raw Pekko public type declarations | 360 |
| raw Pekko `def` declarations | 1594 |
| raw fraktor public type declarations | 87 (`remote-core`: 70 / `remote-adaptor-std`: 17) |
| raw fraktor public method declarations | 350 (`remote-core`: 309 / `remote-adaptor-std`: 41) |
| hard / medium / easy / trivial gap | 8 / 0 / 0 / 0 |

raw declaration count は private / deprecated / JVM 固有 API を含む参考値であり、parity 分母には使わない。

`todo!()` / `unimplemented!()` / `panic!("not implemented")` は remote core / adaptor の production code から検出されない。production code 上の明示 TODO は `modules/remote-core/src/extension/remote.rs` の `remote-redelivery` で、ACK window に基づく再送状態更新が未導入であることを示す。`modules/remote-core/src/wire/primitives.rs` の header placeholder は encode 時の長さ埋め戻しであり、未実装ギャップには分類しない。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core / remote primitives | address、unique address、association、wire PDU、failure detector、watcher state、provider contract、typed config | `modules/remote-core/src/` に整理済み。no_std 側の状態機械と PDU は揃っている | 公開 primitive は強い。ACK/NACK redelivery の runtime 状態更新は残る |
| std / adaptor | TCP listener/client、association runtime、remoting lifecycle、watcher actor、reconnect/backoff、byte payload delivery | `TcpRemoteTransport`、`AssociationRegistry`、`run_inbound_dispatch`、`run_outbound_loop_with_reconnect`、`WatcherActor`、inbound envelope delivery adapter は存在 | bind / handshake / reconnect / quarantine filter / byte payload delivery は動く |
| actor-core integration | serialization registry、ActorRefProvider、DeathWatch、event stream、routing/deploy | misc serializer、scheme provider lookup、remote `ActorRef` materialization、routee expansion、byte payload remote send は接続済み | 任意 actor message serialization、remote DeathWatch 通知、remote deployment が残る |

## カテゴリ別ギャップ

ギャップ表には未対応・部分実装・n/a のみを列挙する。実装済み項目はカテゴリ件数に含めるが、表には出さない。

### 1. Address / identity 実装済み 4/4 (100%)

`Address`, `UniqueAddress`, `RemoteNodeId`, `resolve_remote_address` は実装済み。Pekko の `UniqueAddress(address, uid)` と同じ責務を持つ。

### 2. Failure detector 実装済み 6/6 (100%)

`FailureDetector`, `DeadlineFailureDetector`, `PhiAccrualFailureDetector`, `HeartbeatHistory`, `FailureDetectorRegistry`, `DefaultFailureDetectorRegistry` は実装済み。address-bound detector registry も no_std core に入っている。

### 3. Transport / association / lifecycle 実装済み 17/18 (94%)

`Association`, `AssociationEffect`, `SendQueue`, `QuarantineReason`, `HandshakeValidationError`, `RemoteTransport`, `TcpRemoteTransport`, `AssociationRegistry`, `AssociationShared`, `HandshakeDriver`, `SystemMessageDeliveryState`, `ReconnectBackoffPolicy`, `RestartCounter`, `InboundQuarantineCheck`, lifecycle effect application、byte payload の outbound / inbound delivery は実装済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| ACK/NACK に基づく redelivery state application | `artery/SystemMessageDelivery.scala`, `artery/Control.scala` | 部分実装 | core/extension + std/association | hard | `AckPdu` と `SystemMessageDeliveryState` は存在するが、`Remote` の ACK handler は再送 window / resend state を更新しない |

### 4. Wire protocol / serialization 実装済み 12/13 (92%)

`FrameHeader`, `EnvelopePdu`, `HandshakePdu`, `ControlPdu`, `AckPdu` と各 codec、`MessageContainerSerializer`、`SystemMessageSerializer`、`MiscMessageSerializer` 主要 manifest、manifest-route fallback、`ActorIdentity` remote `ActorRef` restoration、outbound / inbound `maximum_frame_size` enforcement、`Bytes` / `Vec<u8>` payload の envelope encode は実装済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| serializer registry backed arbitrary user payload serialization | `MessageSerializer.scala`, `ArteryMessageSerializer.scala` | 部分実装 | std/provider + std/transport/tcp + actor-core-kernel/serialization | hard | `TcpRemoteTransport` は adapter 所有の `Bytes` / `Vec<u8>` payload を送れるが、任意 `AnyMessage` は fail-fast する。Pekko 相当の serializer registry 経由 encode / decode driver が未配置 |

### 5. Provider / remote actor ref / routing 実装済み 9/11 (82%)

`RemoteActorRef`, `RemoteActorRefProvider` trait、local/no-authority dispatch、loopback authority dispatch、`ActorRefResolveCache` 経由の remote resolve、cache hit/miss event publish、concrete remote `ActorRef` construction、byte payload remote send は実装済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| remote DeathWatch interception | `RemoteActorRefProvider.scala`, `RemoteWatcher.scala` | 部分実装 | std/provider + std/watcher_actor + actor-core-kernel | hard | `watch` / `unwatch` intent は provider にあるが、actor-core DeathWatch への最終接続がない |
| remote deployment daemon / `useActorOnNode` | `RemoteDaemon.scala`, `RemoteDeployer.scala`, `RemoteDeploymentWatcher.scala` | 未対応 | std/provider + actor-core-kernel | hard | remote child actor 作成要求と deployment watcher がない |

### 6. Watcher / DeathWatch runtime 実装済み 5/7 (71%)

`WatcherState`, `WatcherCommand`, `WatcherEffect`, `WatcherActor`, `run_heartbeat_loop`, heartbeat response UID tracking、UID 変更時の rewatch effect は実装済み。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| watcher effects application | `RemoteWatcher.scala` | 部分実装 | std/watcher_actor + actor-core-kernel | hard | `WatcherActor` は effects を `effect_tx` へ流すだけで、`Terminated` / event stream / system message に適用しない |
| `AddressTerminated` integration | `RemoteWatcher.scala`, actor event stream | 未対応 | actor-core-kernel + std/watcher_actor | hard | remote node failure を local DeathWatch へ統合する契約がない |

### 7. Instrumentation / config / logging 実装済み 9/9 (100%)

`RemotingLifecycleState`, `StdRemoting`, `EventPublisher`, `RemoteLogMarker`, `RemoteInstrument`, `RemotingFlightRecorder`, `RemoteAuthoritySnapshot`、主要 `RemoteConfig` builder は実装済み。`bind_hostname` / `bind_port` / `inbound_lanes` / `outbound_lanes` / `maximum_frame_size` / `buffer_pool_size` / `untrusted_mode` / log toggle / outbound queue / remove-quarantined / outbound restart budget / inbound restart budget / large-message destinations / compression config は現行コードで確認済み。

large-message destinations は outbound queue selection に反映済みで、`outbound_large_message_queue_size` は独立 queue の上限として使われる。`inbound_lanes` / `outbound_lanes` は TCP transport の dispatch / writer lanes に適用済みである。`RemoteCompressionConfig` は保持のみの契約を維持し、compression advertisement / table application は Phase 3 serializer registry / payload codec の future item として扱う。

### 8. Reliability / lifecycle adapter 実装済み 2/4 (50%)

`InboundQuarantineCheck` と `RestartCounter` は実装済み。shutdown / DeathWatch 前 flush は残る。

| Pekko API / 契約 | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|------------------|-----------|-------------|----------|--------|------|
| `FlushOnShutdown` | `artery/FlushOnShutdown.scala` | 未対応 | std/extension_installer + std/association | hard | `shutdown_flush_timeout` 設定はあるが、association に termination hint を送り ack を待つ driver がない |
| `FlushBeforeDeathWatchNotification` | `artery/FlushBeforeDeathWatchNotification.scala` | 未対応 | std/watcher_actor + std/association | hard | DeathWatch 通知前に対象 association を flush する契約がない |

### 9. Internal helpers / cache 実装済み 2/2 (100%)

`ActorRefResolveCache` と `RemoteActorRefResolveCacheEvent` / `RemoteActorRefResolveCacheOutcome` は実装済み。`StdRemoteActorRefProvider` から hit/miss event も publish される。

## 対象外 n/a

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

固定スコープ概念カバレッジは 89.2% で、残るギャップは 8 件の `hard` gap に集中している。API / 実動作ギャップがまだ支配的なため、内部モジュール構造の詳細分析は後続フェーズとする。

| 構造観点 | 現状 | 次に見るべき点 |
|----------|------|----------------|
| `core::wire` と actor-core-kernel serialization の境界 | `Bytes` / `Vec<u8>` payload は remote envelope に載るが、任意 `AnyMessage` payload は serializer registry を通らない | `SerializationDelegator` を provider / transport / association runtime のどの層から呼ぶか |
| ACK/NACK redelivery の境界 | `AckPdu` と `SystemMessageDeliveryState` はあるが、ACK handler は runtime state を更新しない | resend window、ack coalescing、association reconnect 時の再送責務を core state と std driver に分けるか |
| provider と actor-core-kernel DeathWatch の境界 | remote `ActorRef` materialization と byte payload send は実装済み | `watch` / `unwatch` intent を actor-core DeathWatch / event stream にどう接続するか |
| watcher effect application | pure `WatcherState` と tokio actor はある | `NotifyTerminated` / `NotifyQuarantined` / `RewatchRemoteTargets` を actor-core に適用する adapter |
| flush 系契約 | `shutdown_flush_timeout` 設定だけ先行 | `FlushOnShutdown` / `FlushBeforeDeathWatchNotification` を core state と std driver に分けるか |

## 実装優先度

この節では、上で列挙したギャップだけを Phase に再配置する。

### Phase 1: trivial / easy

該当なし。公開 API surface を単純に足すだけで解消できる未実装ギャップは現時点ではない。

### Phase 2: medium

該当なし。Phase 2 medium gap だった consistent-hashing pool remote router serialization と advanced Artery settings runtime application は解消済みである。compression advertisement / table application は任意 actor message serialization と同じ serializer registry 境界で扱うため、Phase 2 の完了条件には含めない。

### Phase 3: hard

B 方針により、Pekko wire byte compatibility 固有の項目は Phase 3 から外す。Phase 3 は fraktor-rs 独自 wire 上で remote actor messaging と lifecycle を成立させるための hard gap に限定する。

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| ACK/NACK に基づく redelivery state application | core/extension + std/association | 3 |
| serializer registry backed arbitrary user payload serialization | std/provider + std/transport/tcp + actor-core-kernel/serialization | 4 |
| remote DeathWatch interception | std/provider + std/watcher_actor + actor-core-kernel | 5 |
| remote deployment daemon / `useActorOnNode` | std/provider + actor-core-kernel | 5 |
| watcher effects application | std/watcher_actor + actor-core-kernel | 6 |
| `AddressTerminated` integration | actor-core-kernel + std/watcher_actor | 6 |
| `FlushOnShutdown` | std/extension_installer + std/association | 8 |
| `FlushBeforeDeathWatchNotification` | std/watcher_actor + std/association | 8 |

## まとめ

remote は address primitives、association state machine、failure detector + registry、typed `RemoteConfig`、TCP transport shell、inbound quarantine、restart budget、watcher UID protocol、resolve cache、remote `ActorRef` materialization、主要 misc serialization、byte payload の二ノード配送までカバー済みで、基礎部品の parity は進んでいる。

Phase 1 / Phase 2 の未実装ギャップは現時点ではない。

主要ギャップは Phase 3 の任意 actor message serialization、ACK/NACK redelivery、remote DeathWatch / `AddressTerminated` 統合、remote deployment、flush lifecycle に集中している。byte payload delivery と Phase 2 medium gap は実装済みになったため、次のボトルネックは「送信経路そのもの」ではなく、Pekko の serializer registry と DeathWatch / lifecycle 契約を fraktor-rs の `actor-core-kernel` 境界へどう接続するかである。
