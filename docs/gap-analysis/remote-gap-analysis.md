# remote モジュール ギャップ分析

更新日: 2026-04-27 (6th edition / Phase 2 medium 完了 + Phase 3 hard 再確認 + 方針判断追加版)

## 比較スコープ定義

この調査は、Apache Pekko remote 配下の raw API 数をそのまま移植対象にするものではない。fraktor-rs の `remote` では、Pekko Artery compatible な remote actor transport 契約を対象にし、classic remoting / JVM 実装技術 / testkit は parity 分母から除外する。

公開境界は `modules/remote-core/src/core/` (Pekko Artery 互換 core) と `modules/remote-adaptor-std/src/std/` (tokio adapter)。前回 5th edition と同じ namespace 前提を維持する。

### 対象に含めるもの

| 領域 | fraktor-rs | Pekko 参照 |
|------|------------|------------|
| remote core | `modules/remote-core/src/core/` | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/` |
| Artery transport contract | `modules/remote-core/src/core/transport/`, `association/`, `wire/` | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/artery/` |
| std TCP adapter | `modules/remote-adaptor-std/src/std/tcp_transport/`, `association_runtime/` | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/artery/tcp/` の TCP 契約 |
| remote actor ref provider | `modules/remote-core/src/core/provider/`, `modules/remote-adaptor-std/src/std/provider/` | `RemoteActorRefProvider.scala` / `RemoteActorRef` 相当 |
| failure detector / watcher | `modules/remote-core/src/core/failure_detector/`, `watcher/`, `modules/remote-adaptor-std/src/std/watcher_actor/` | `FailureDetector*.scala`, `RemoteWatcher.scala` |
| serialization 接続点 | `modules/actor-core/src/core/kernel/serialization/`, `modules/remote-core/src/core/wire/` | `remote/serialization/` の remote transport に必要な契約 |
| lifecycle / instrumentation | `modules/remote-core/src/core/extension/`, `instrument/`, `modules/remote-adaptor-std/src/std/extension_installer/` | `RemotingLifecycleEvent.scala`, `RemoteLogMarker.scala`, `RemoteInstrument.scala` |

### 対象から除外するもの

| 除外項目 | 理由 |
|----------|------|
| classic remoting / `Endpoint.scala` / `AckedDelivery.scala` | Pekko 側でも deprecated。Artery 互換の分母には入れない |
| `transport/netty/` 配下、`PekkoProtocolTransport.scala`, `PekkoPduCodec.scala`, `AbstractTransportAdapter.scala` | classic transport stack |
| `FailureInjectorTransportAdapter`, `ThrottlerTransportAdapter`, `TestTransport` | test / fault injection 用 |
| Aeron UDP transport (`artery/aeron/*`) | JVM Aeron 固有実装、Rust std TCP adapter とは別物 |
| TLS / `SSLEngineProvider` / `ConfigSSLEngineProvider` / `RotatingKeysSSLEngineProvider` / `security/provider/*` | Java `SSLEngine` / HOCON / classloader に依存する完全互換は除外。Rust TLS adapter が必要なら別スコープ |
| `JavaSerializer` / Jackson module 完全互換 | serialization contract との接続点だけ対象 |
| HOCON provider loading / `FailureDetectorLoader` 動的ロード / JVM classloader | JVM 設定ロード方式に依存 |
| JFR `artery/jfr/Events.scala`, `JFRRemotingFlightRecorder.scala` | JFR は JVM 固有。Rust 側は `RemotingFlightRecorder` (ring buffer) で代替 |
| remote testkit / multi-node-testkit / remote-tests / `TestStage.scala` | runtime API ではない |
| `RemoteMetricsExtension`, `AddressUidExtension`, `BoundAddressesExtension` | JVM 拡張ローダ依存。同等情報は `RemotingLifecycleState` / `RemoteAuthoritySnapshot` で再現済み |
| `EnvelopeBufferPool`, `ObjectPool`, `FixedSizePartitionHub` | JVM GC 回避目的の最適化用 buffer pool。Rust では割り当て戦略が異なるため完全互換は不要 |
| `ImmutableLongMap`, `LruBoundedCache` | internal collection helper。Rust では `hashbrown` / `BTreeMap` 等で代替 |
| `ProtobufSerializer` | Pekko 内部で protobuf wire encode を分離するための adapter。fraktor は独自 binary codec を採用 |

### raw 抽出値の扱い

`references/pekko/remote` の raw 抽出は依然として public / private / deprecated / JVM 固有を大量に含む。これらは参考値に留め、固定スコープの parity カバレッジ分母には使わない。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 約 58 |
| fraktor-rs 固定スコープ対応概念 | 約 43 |
| 固定スコープ概念カバレッジ | 約 43/58 (74%) |
| raw public type declarations | 79（core: 57, std: 22） |
| raw public method declarations | 290（core: 210, std: 80） |
| hard / medium / easy / trivial gap | 14 / 1 / 0 / 0 (Phase 2 medium は前回 10/10 完了。今回 advanced settings の lanes/frame size/untrusted/log を再分割した結果 1 件残置) |

前回 5th edition との差分:

- 前回 `domain/` 前提の評価が含まれる箇所はすべて `core/` に統一
- Phase 2 medium 10 件はすべてコード上で完了確認済み (`FailureDetectorRegistry`, `RemoteTransport.start` 実 bind, handshake validation, per-peer routing, system message delivery, reconnect/backoff, MessageContainerSerializer, SystemMessageSerializer, MiscMessageSerializer subset, advanced settings の timing 部分)
- Phase 3 hard 12 件は全件未着手のまま (`build_envelope_frame()` の `Bytes::new()` placeholder、`WireFrame::Envelope` の debug log only inbound dispatch、`RemoteSenderBuildFailed` を返す provider、watcher effect が `effect_tx` までで止まる経路、Pekko Artery framing 非互換、compression / RemoteRouterConfig / RemoteDaemon 不在 を確認)
- 5th edition で扱っていなかった Phase 3 hard 候補 (DaemonMsgCreateSerializer / FlushOnShutdown / FlushBeforeDeathWatchNotification / InboundQuarantineCheck / ActorRefResolveCache / RestartCounter) を表に明示
- `RemoteConfig` の advanced settings は timing 部分のみ実装済み。lanes / maximum_frame_size / untrusted_mode / log toggles / bind_hostname / bind_port / buffer_pool_size は依然不足のため medium 1 件として再計上

`todo!()` / `unimplemented!()` / `panic!("not implemented")` は remote core / adaptor から検出されない。一方で `Phase B minimum-viable` や placeholder コメントは複数残り、未完了箇所として扱う必要がある。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core / remote primitives | `Address`, `UniqueAddress`, association state machine (handshake validation 含む), wire PDU, watcher state, failure detector + registry, provider contract, advanced timing settings | `modules/remote-core/src/core/` に揃う。`HandshakeValidationError::RejectedInState` で Idle/Gated/Quarantined を型安全に拒否、`DefaultFailureDetectorRegistry` も実装済み | 公開面は十分。残るのは compression / DaemonMsgCreate 等の hard 領域と advanced settings の追加 field |
| std / adaptor | `RemoteTransport`, TCP listener/client, per-peer association runtime, remoting lifecycle wiring, reconnect-with-backoff outbound loop | `StdRemoting`, `TcpRemoteTransport`, `association_runtime`, `watcher_actor` を配置。`RemoteTransport::start` は `server.start(...)` 経由で実 bind、`run_outbound_loop_with_reconnect` も完成 | bind / handshake / reconnect は機能。残りは inbound envelope delivery、payload serialization、remote ActorRef wiring |
| actor-core integration | event stream, serializer registry, DeathWatch | lifecycle event publish, MessageContainerSerializer, SystemMessageSerializer, MiscMessageSerializer (Identify subset) は接続済み。remote DeathWatch、`AddressTerminated`、ActorIdentity、RemoteRouterConfig は未接続 | 大きいギャップが残る (Phase 3 hard) |

## カテゴリ別ギャップ

ギャップ表には未対応・部分実装・n/a のみを列挙する。実装済み項目はカテゴリ件数に含めるが、表には出さない。

### 1. Address / identity　✅ 実装済み 4/4 (100%)

`Address`, `UniqueAddress`, `RemoteNodeId`, `resolve_remote_address` は実装済み。Pekko の `UniqueAddress` と同じく `(Address, uid)` を保持する。

### 2. Failure detector　✅ 実装済み 6/6 (100%)

5th edition で残っていた `FailureDetectorRegistry[A]` / `DefaultFailureDetectorRegistry[A]` は `modules/remote-core/src/core/failure_detector/{failure_detector_registry.rs,default_failure_detector_registry.rs}` で実装済み。`PhiAccrualFailureDetector`, address-bound metadata, `DeadlineFailureDetector`, `HeartbeatHistory` も継続して提供。

### 3. Transport / association / lifecycle　✅ 実装済み 14/15 (93%)

`Association`, `AssociationEffect`, `SendQueue`, `QuarantineReason`, `HandshakeValidationError` (`RejectedInState` バリアント含む), `HandshakeRejectedState`, `RemoteTransport` trait, `TcpRemoteTransport`, `StdRemoting`, `ListenStarted` publish, `AssociationRegistry`, `HandshakeDriver`, `SystemMessageDeliveryState`, `ReconnectBackoffPolicy`, `run_outbound_loop_with_reconnect`, `EventPublisher` 経由の lifecycle effect 適用は実装済み。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| inbound envelope delivery | `ArteryTcpTransport.scala:405`, `MessageDispatcher.scala:33` | 部分実装 | std/association_runtime + std/provider | hard | `WireFrame::Envelope(_pdu)` は inbound dispatcher で `tracing::debug!` のみ。local actor / mailbox へ配送しない (`inbound_dispatch.rs:41-44` で確認) |

### 4. Wire protocol / serialization　✅ 実装済み 9/15 (60%)

`MessageContainerSerializer` (`modules/actor-core/src/core/kernel/serialization/builtin/message_container_serializer.rs`)、`SystemMessageSerializer`、`MiscMessageSerializer` (`Address` / `UniqueAddress` / `Identify` manifest 対応)、`ThrowableNotSerializableError` は実装済み。delegator manifest-route fallback も整備済み。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| Pekko Artery TCP framing | `artery/tcp/TcpFraming.scala:34` | 未対応 | std/tcp_transport | hard | fraktor は独自 `length(4) + version(1) + kind(1)` frame (`frame_codec.rs:13-20`)。Pekko の `AKKA` magic / stream id / little-endian framing と非互換 |
| `ArteryMessageSerializer` control protocol | `serialization/ArteryMessageSerializer.scala:56` | 部分実装 | core/wire + actor-core/serialization | hard | `HandshakePdu` / `ControlPdu` / `AckPdu` はあるが、Pekko manifest / protobuf control 互換ではない |
| message payload serialization into envelope | `MessageSerializer.scala:81`, `ArteryMessageSerializer.scala:178` | 部分実装 | std/tcp_transport + actor-core/serialization | hard | `build_envelope_frame()` が `Bytes::new()` placeholder を使う (`tcp_transport/base.rs:231-241` で確認) |
| `MiscMessageSerializer` 残り (`ActorIdentity`, `RemoteRouterConfig`, `Status.Failure`, `RemoteScope`, etc.) | `serialization/MiscMessageSerializer.scala:37` 以下 | 部分実装 | actor-core/serialization | medium | 現状は `Address` / `UniqueAddress` / `Identify` のみ。`ActorIdentity` は ActorRef path serialization 拡張、`RemoteRouterConfig` は routing layer 拡張待ち |
| `DaemonMsgCreateSerializer` | `serialization/DaemonMsgCreateSerializer.scala` | 未対応 | actor-core/serialization | hard | remote daemon 経由の child actor 作成要求の wire 表現。`RemoteDaemon` / `useActorOnNode` (カテゴリ5) と一体で必要 |
| Artery compression protocol (`CompressionProtocol`, `CompressionTable`, `InboundCompressions`, `TopHeavyHitters`) | `artery/compress/*`, `artery/Codecs.scala` | 未対応 | core/wire + core/association | hard | wire protocol 上の actor ref / manifest compression 経路が不在 |

### 5. Provider / remote actor ref / routing　✅ 実装済み 3/8 (38%)

`RemoteActorRef`, `RemoteActorRefProvider` trait, local/remote dispatch ルートを持つ `StdRemoteActorRefProvider`, `RemoteActorRefSender` は存在。実体化と送信経路が未完了。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| concrete remote `ActorRef` construction | `RemoteActorRefProvider.scala:161,673` | 部分実装 | std/provider | hard | remote branch は `provider/dispatch.rs:100` で `Err(StdRemoteActorRefProviderError::RemoteSenderBuildFailed)` を返す |
| remote send path | `RemoteActorRefProvider.scala:763` | 部分実装 | std/provider + std/tcp_transport | hard | `RemoteActorRefSender` はあるが、payload serialization と watch integration が placeholder。実 transport.send まで結線されていない |
| remote DeathWatch interception | `RemoteActorRefProvider.scala:739` | 部分実装 | std/provider + std/watcher_actor + actor-core | hard | `WatcherCommand::Watch` / `Unwatch` intent はあるが `Terminated(AddressTerminated)` 統合がない |
| `RemoteRouterConfig` | `routing/RemoteRouterConfig.scala:47` | 未対応 | actor-core/routing + core/provider | medium | actor-core routing はあるが、remote node list に pool routee を展開する契約がない |
| remote deployment daemon / `useActorOnNode` | `RemoteDaemon.scala`, `RemoteDeployer.scala`, `RemoteDeploymentWatcher.scala` | 未対応 | std/provider + actor-core | hard | remote child actor 作成要求と deployment watch の経路がない |

### 6. Watcher / DeathWatch runtime　✅ 実装済み 3/6 (50%)

`WatcherState`, `WatcherCommand`, `WatcherEffect`, `WatcherActor`, `run_heartbeat_loop` は実装済み。watch bookkeeping と detector tick は動くが、actor-core への最終適用が不足。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| heartbeat response protocol | `RemoteWatcher.scala:56,57` | 部分実装 | core/wire + std/watcher_actor | medium | heartbeat tick / receive はあるが、response uid / actor-system UID 検証は未接続 |
| watcher effects application | `RemoteWatcher.scala:103` | 部分実装 | std/watcher_actor + actor-core | hard | `WatcherActor` は `WatcherEffect` を `effect_tx` (`watcher_actor/base.rs:62`) に流すだけで、`Terminated` / event stream 適用がない |
| `AddressTerminated` integration | `RemoteWatcher.scala:103`, `SystemMessageSerializer.scala` | 未対応 | actor-core + std/watcher_actor | hard | remote node failure を local DeathWatch へ統合する契約がない |

### 7. Instrumentation / config / logging　✅ 実装済み 6/7 (86%)

`RemotingLifecycleState`, `EventPublisher`, `ListenStarted` publish, `RemoteLogMarker`, `RemoteInstrument`, `RemotingFlightRecorder`, advanced settings の timing 部分は実装済み。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| advanced Artery settings 残り (lanes / maximum_frame_size / untrusted_mode / log toggles / bind_hostname / bind_port / buffer_pool_size) | `ArterySettings.scala` の `Advanced` block | 未対応 | core/config | medium | `RemoteConfig` は 17 field (handshake / ack / restart / system message timing) まで。lanes / frame size / untrusted / bind / log は未追加 |

### 8. Reliability / lifecycle adapter　✅ 実装済み 0/4 (0%)

5th edition では「instrumentation」と一体で扱っていたが、今回 Pekko Artery 側の reliability adapter 群を独立カテゴリとして再分割した。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `FlushOnShutdown` | `artery/FlushOnShutdown.scala` | 未対応 | std/extension_installer + std/association_runtime | hard | shutdown 時の send queue / outbound buffer flush 駆動。`shutdown_flush_timeout` 設定はあるが driver 不在 |
| `FlushBeforeDeathWatchNotification` | `artery/FlushBeforeDeathWatchNotification.scala` | 未対応 | std/watcher_actor + std/association_runtime | hard | DeathWatch 通知前に対象 actor 宛 outbound を flush する Pekko 固有契約 |
| `InboundQuarantineCheck` | `artery/InboundQuarantineCheck.scala` | 未対応 | std/association_runtime | medium | quarantined association からの inbound を破棄するフィルタ。現状の inbound dispatcher にこの分岐が無い |
| `RestartCounter` | `artery/RestartCounter.scala` | 部分実装 | std/association_runtime | medium | `ReconnectBackoffPolicy::max_restarts` で再接続上限は管理済みだが、Pekko の (count, deadline) ベース counter helper 相当の独立型はない |

### 9. Internal helpers / cache　✅ 実装済み 0/2 (0%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `ActorRefResolveCache` | `serialization/ActorRefResolveCache.scala` | 未対応 | actor-core/serialization + std/provider | medium | 同一 actor path の再 resolve コストを抑える LRU。remote send path が完成すれば必要になる |
| `RemoteActorRef` 解決の cache hit / miss 計測 | `RemoteActorRefProvider.scala:330` 周辺 | 未対応 | std/provider | medium | 上記 cache の hit/miss を `EventPublisher` 経由で観測する経路 |

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
| `artery/jfr/Events.scala`, `JFRRemotingFlightRecorder.scala` | JVM Flight Recorder 固有。Rust 側は `RemotingFlightRecorder` (ring buffer) で代替 |
| HOCON provider loading / `FailureDetectorLoader` 動的ロード / JVM classloader | JVM 設定ロード方式 |
| `TestStage`, multi-node-testkit, remote-tests | runtime API ではない |

## Phase 3 着手前に決めるべき方針判断

Phase 3 hard 14 件の実装は、以下の方針判断 1 つに強く依存する。Phase 2 medium を takt で回す前に明文化しておくと、advanced settings の命名や serializer manifest の選定が方針側に揃う分、後続の手戻りが減る。

### Q. Pekko Artery と wire-protocol parity を目指すか

| 選択肢 | wire 互換 | 影響範囲 | 主なトレードオフ |
|--------|-----------|----------|------------------|
| A. protocol parity (Pekko Artery と相互運用可能) | TCP framing で `AKKA` magic + stream id、`ArteryMessageSerializer` の protobuf control PDU、`CompressionTable` の wire 表現すべて Pekko 互換 | `tcp_transport/frame_codec.rs` 全書き換え、`core/wire/*` の PDU 再設計、`DaemonMsgCreateSerializer` も Pekko protobuf manifest と整合 | Pekko クラスタとの相互運用が得られる。Rust 側 binary codec 設計の自由度を失う |
| B. responsibility parity のみ (責任分割は Pekko、wire は独立) | 現状維持。`length(4) + version(1) + kind(1)` framing と独自 PDU / manifest | wire layer の自由度を保ち、framing 互換用コードを書かない | Pekko クラスタとは相互運用しない (fraktor ノード同士のみ) |

`lib.rs` の "Pekko Artery compatible" は責任分割 (B) の意味で書かれている可能性が高いが、現時点で明文化されていない。Phase 3 で `ArteryMessageSerializer` / `CompressionProtocol` / `DaemonMsgCreateSerializer` を書く前に openspec proposal で確定させる必要がある。

A を選ぶ場合の影響を受ける項目: カテゴリ 4 (Wire protocol / serialization) の Phase 3 hard 5 件 (Pekko Artery TCP framing / `ArteryMessageSerializer` / payload serialization / `DaemonMsgCreateSerializer` / Compression protocol)、advanced settings の `transport` / compression toggle の命名。

B を選ぶ場合の影響を受ける項目: 同 5 件は「Pekko 由来の責任分割を踏襲しつつ独自 binary 実装」となり、命名 / 内部 PDU 形は自由。advanced settings は fraktor 独自命名で良い。

### 早すぎる refactor を避ける箇所

以下は Phase 3 hard を実装してから自然に正しい責務境界が見える。いま先回りで refactor しない。

| 観点 | いま触らない理由 | 検討するタイミング |
|------|------------------|--------------------|
| payload serialization の owner (`build_envelope_frame` の置き場 — wire / association_runtime / provider のどれか) | remote send path 実装時に正しい呼び出し位置が決まる | Phase 3 hard の "remote send path" 着手時 |
| 設定だけ先行している箇所 (`shutdown_flush_timeout`, `outbound_max_restarts` 等) | driver 側 (`FlushOnShutdown`, `RestartCounter`) が決まれば設定との配線も自動的に決まる | Phase 3 hard の "FlushOnShutdown" / Phase 2 medium の "RestartCounter" 着手時 |
| `RemoteConfig` を 1 構造体にまとめるか、transport 種別ごとに分けるか | TLS / bind / lanes を実際に追加するときに肥大化具合が分かる | Phase 2 medium の "advanced settings 残り" 着手時 |

これらは API 面が埋まったあとの「内部モジュール構造ギャップ分析」フェーズで一括整理する (本レポート末尾の構造観点表を起点にする)。

## 実装優先度

この節では、上で列挙したギャップ (Phase 2 medium 1 件 + Phase 3 hard 14 件) を Phase に再配置する。新規提案は追加せず、カテゴリ別ギャップ表に存在する項目だけを並べる。

### Phase 1: trivial / easy

該当なし。Phase 1 / Phase 2 medium のうち trivial / easy 相当はすべて 5th edition 時点で完了済み。

### Phase 2: medium

| 項目 | 実装先層 | 根拠 (カテゴリ) |
|------|----------|----------------|
| `MiscMessageSerializer` 残り (`ActorIdentity`, `RemoteRouterConfig`, `Status.Failure`, `RemoteScope` 等) | actor-core/serialization | 4 |
| advanced Artery settings 残り (lanes / maximum_frame_size / untrusted_mode / log toggles / bind_hostname / bind_port / buffer_pool_size) | core/config | 7 |
| heartbeat response protocol (response uid / actor-system UID 検証) | core/wire + std/watcher_actor | 6 |
| `InboundQuarantineCheck` | std/association_runtime | 8 |
| `RestartCounter` (Pekko 互換 helper) | std/association_runtime | 8 |
| `RemoteRouterConfig` | actor-core/routing + core/provider | 5 |
| `ActorRefResolveCache` | actor-core/serialization + std/provider | 9 |
| RemoteActorRef 解決の cache hit / miss 計測 | std/provider | 9 |

### Phase 3: hard

| 項目 | 実装先層 | 根拠 (カテゴリ) |
|------|----------|----------------|
| inbound envelope delivery | std/association_runtime + std/provider | 3 |
| Pekko Artery TCP framing | std/tcp_transport | 4 |
| `ArteryMessageSerializer` control protocol | core/wire + actor-core/serialization | 4 |
| message payload serialization into envelope | std/tcp_transport + actor-core/serialization | 4 |
| `DaemonMsgCreateSerializer` | actor-core/serialization | 4 |
| Artery compression protocol (`CompressionProtocol`, `CompressionTable`, `InboundCompressions`, `TopHeavyHitters`) | core/wire + core/association | 4 |
| concrete remote `ActorRef` construction | std/provider | 5 |
| remote send path | std/provider + std/tcp_transport | 5 |
| remote DeathWatch interception | std/provider + std/watcher_actor + actor-core | 5 |
| remote deployment daemon / `useActorOnNode` | std/provider + actor-core | 5 |
| watcher effects application | std/watcher_actor + actor-core | 6 |
| `AddressTerminated` integration | actor-core + std/watcher_actor | 6 |
| `FlushOnShutdown` | std/extension_installer + std/association_runtime | 8 |
| `FlushBeforeDeathWatchNotification` | std/watcher_actor + std/association_runtime | 8 |

## 内部モジュール構造ギャップ

今回も API / 実動作ギャップが支配的なため、内部モジュール構造ギャップの詳細分析は省略する。特に以下が end-to-end remote actor delivery を阻む直接の壁であり、責務分割の最適化より先に公開契約と end-to-end 経路を閉じる段階である。

- remote `ActorRef` 実体化
- `build_envelope_frame()` の payload serialization
- `WireFrame::Envelope` の inbound delivery
- watcher effects の actor-core 適用 (`AddressTerminated` 統合)

次版で構造分析へ進む場合の観点は以下になる。

| 構造観点 | 現状 | 次に見るべき点 |
|----------|------|----------------|
| `core::wire` と actor-core serialization の境界 | `wire` は独自 PDU を持つが byte payload が placeholder | `AnyMessage` → serialized bytes の責務をどこへ置くか (delegator 経由か新 trait か) |
| `association_runtime` の責務 | handshake / outbound / inbound / effect application / reconnect が揃うが、peer registry と delivery adapter が未接続 | per-peer registry と inbound delivery adapter を分けるか |
| provider と actor-core provider の境界 | local/remote dispatch まではあるが remote branch が `RemoteSenderBuildFailed` | `ActorSystemState` 依存をどこに閉じるか、RemoteActorRef 実体化の owner |
| watcher effect の適用先 | pure `WatcherState` と tokio actor はある | `Terminated` / event stream / system message への接続点と `AddressTerminated` 経路 |
| flush 系契約の置き場 | `shutdown_flush_timeout` 設定だけあり driver が無い | `FlushOnShutdown` / `FlushBeforeDeathWatchNotification` を core にロジック・std に driver で分離するか |

## まとめ

remote の公開境界は `core/` に整理され、address primitives、association state machine (handshake validation 含む)、failure detector + registry、`DeadlineFailureDetector`、address-bound `PhiAccrualFailureDetector`、`RemoteLogMarker`、`ListenStarted` publish、handshake 拒否 (`RejectedInState`)、reconnect-with-backoff outbound loop、MessageContainerSerializer / SystemMessageSerializer / MiscMessageSerializer (Identify subset) まで実装済み。前回 5th edition の Phase 2 medium 10 件はコード上で完了確認済みである。

低コストで parity を前進できる Phase 2 medium は 8 件: `MiscMessageSerializer` 残り、advanced Artery settings の追加 field、heartbeat response 検証、`InboundQuarantineCheck`、`RestartCounter` helper、`RemoteRouterConfig`、`ActorRefResolveCache`、cache hit/miss 観測。

主要ギャップは Phase 3 hard で 14 件: end-to-end の inbound envelope delivery、Pekko Artery TCP framing 互換、payload serialization、`DaemonMsgCreateSerializer`、compression protocol、remote `ActorRef` 実体化、remote send path、remote DeathWatch / `AddressTerminated` 統合、remote deployment daemon、watcher effects 適用、`FlushOnShutdown` / `FlushBeforeDeathWatchNotification`。これらが揃うまでは end-to-end remote actor delivery が動作しないため、内部構造の細部比較よりも公開契約と実配送経路の未完成部分を先に閉じるのが妥当である。
