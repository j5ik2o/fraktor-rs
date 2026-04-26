# remote モジュール ギャップ分析

更新日: 2026-04-26 (5th edition / `core` namespace 再評価版)

## 比較スコープ定義

この調査は、Apache Pekko remote 配下の raw API 数をそのまま移植対象にするものではない。fraktor-rs の `remote` では、Pekko Artery compatible な remote actor transport 契約を対象にし、classic remoting / JVM 実装技術 / testkit は parity 分母から除外する。

前回レポートの `modules/remote-core/src/domain/` 前提は現状と一致しない。現在の公開境界は `modules/remote-core/src/core/` であり、このレポートはその前提で再評価している。

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
| Netty transport / Aeron UDP 完全互換 | JVM / Netty / Aeron 固有の transport 実装。Rust std TCP adapter とは別物 |
| TLS / `SSLEngineProvider` 完全互換 | Java `SSLEngine` / HOCON / classloader に依存する実装互換は除外。Rust TLS adapter が必要なら別スコープ |
| Java serialization / Jackson module そのもの | serialization contract との接続点のみ対象 |
| HOCON provider loading / JVM dynamic access / classloader check | JVM 設定ロード方式に依存するため対象外 |
| JFR / Flight Recorder event class 完全互換 | JFR は JVM 固有。Rust 側は ring-buffer flight recorder で代替 |
| remote testkit / multi-node-testkit / remote-tests | runtime API ではない |

### raw 抽出値の扱い

`references/pekko/remote` の raw 抽出は依然として public / private / deprecated / JVM 固有を大量に含む。これらは参考値に留め、固定スコープの parity カバレッジ分母には使わない。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 約 54 |
| fraktor-rs 固定スコープ対応概念 | 約 33 |
| 固定スコープ概念カバレッジ | 約 33/54 (61%) |
| raw public type declarations | 72（core: 52, std: 20） |
| raw public method declarations | 254（core: 187, std: 67） |
| hard / medium / easy / trivial gap | 8 / 8 / 0 / 0 |

前回から改善された点は明確で、`remote-core` の公開境界は `core` に整理され、`DeadlineFailureDetector`、address-bound な `PhiAccrualFailureDetector`、`RemoteLogMarker`、`ListenStarted` event publish、association effects の lifecycle publish は実装済みになっている。

一方で、実動作の観点ではまだ **end-to-end の remote actor delivery が未完成** である。特に次が支配的なボトルネックとして残る。

- `StdRemoteActorRefProvider::actor_ref` の remote branch が依然として `RemoteSenderBuildFailed` を返す
- `TcpRemoteTransport::build_envelope_frame` が payload を `Bytes::new()` の placeholder で送る
- `run_inbound_dispatch` が `WireFrame::Envelope` を観測ログに留め、local actor へ配送しない
- watcher effects が actor-core の `Terminated` / system message 経路へ接続されていない

`todo!()` / `unimplemented!()` / `panic!("not implemented")` は remote core / adaptor から検出されなかった。一方で `Phase B minimum-viable` や placeholder コメントは多数残っており、実装上の未完了箇所として扱う必要がある。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core / remote primitives | `Address`, `UniqueAddress`, association state machine, wire PDU, watcher state, failure detector, provider contract | `modules/remote-core/src/core/` に一通り揃う。`PhiAccrualFailureDetector` は address-bound constructor、`DeadlineFailureDetector`、`RemoteLogMarker` も存在 | 公開面は中程度以上。抽象 registry / serializer / remote deployment が不足 |
| std / adaptor | `RemoteTransport`, TCP listener/client, per-peer association runtime, remoting lifecycle wiring | `modules/remote-adaptor-std/src/std/` に `StdRemoting`, `TcpRemoteTransport`, `association_runtime`, `watcher_actor` を配置 | 骨格はあるが実 bind / inbound delivery / remote ActorRef wiring は未完 |
| actor-core integration | event stream, serializer registry, DeathWatch | lifecycle event publish は接続済み。remote-specific serializer / remote DeathWatch / AddressTerminated は未接続 | 大きいギャップが残る |

## カテゴリ別ギャップ

ギャップ表には未対応・部分実装・n/a のみを列挙する。実装済み項目はカテゴリ件数に含めるが、表には出さない。

### 1. Address / identity　✅ 実装済み 4/4 (100%)

`Address`, `UniqueAddress`, `RemoteNodeId`, `resolve_remote_address` は実装済み。Pekko の `UniqueAddress` と同じく `(Address, uid)` を保持する。

### 2. Failure detector　✅ 実装済み 4/6 (67%)

前回との差分として、`DeadlineFailureDetector` は実装済みになり、address-bound detector metadata も `PhiAccrualFailureDetector::new(address, ...)` で満たしている。したがって、以前の `FailureDetectorWithAddress` ギャップは閉じたと評価してよい。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `FailureDetectorRegistry[A]` | `FailureDetectorRegistry.scala:30` | 未対応 | core/failure_detector | medium | `WatcherState` 内部に detector map はあるが、汎用 registry contract はない |
| `DefaultFailureDetectorRegistry[A]` | `DefaultFailureDetectorRegistry.scala:27` | 未対応 | core/failure_detector | medium | registry API 不在のため同時に未実装 |

実装済みとして扱うもの: `PhiAccrualFailureDetector`, address-bound detector metadata, `DeadlineFailureDetector`, `HeartbeatHistory`。

### 3. Transport / association / lifecycle　✅ 実装済み 8/14 (57%)

`Association`, `AssociationEffect`, `SendQueue`, `QuarantineReason`, `RemoteTransport` trait, `TcpRemoteTransport`, `StdRemoting`, `ListenStarted` publish は存在する。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `RemoteTransport.start` が実 listener を起動する契約 | `RemoteTransport.scala:84`, `ArteryTcpTransport.scala:118` | 部分実装 | std/tcp_transport + std/extension_installer | medium | `TcpRemoteTransport::start_async()` はあるが、`StdRemoting::start()` / installer 経路では未使用。同期 `start()` は `running = true` を立てるだけ |
| handshake validation / retry / liveness probe | `Handshake.scala:37`, `Handshake.scala:63` | 部分実装 | core/association + std/association_runtime | medium | `run_inbound_dispatch` は handshake origin を `RemoteNodeId` 化するが、uid / to / retry / liveness probe の検証がない |
| per-peer inbound association routing | `ArteryTransport.scala:280`, `Association.scala:1131` | 部分実装 | std/association_runtime | medium | `run_inbound_dispatch` は単一 `AssociationShared` に流すだけで、registry lookup がない |
| inbound envelope delivery | `ArteryTcpTransport.scala:405`, `MessageDispatcher.scala:33` | 部分実装 | std/association_runtime + std/provider | hard | `WireFrame::Envelope` は debug log のみ。local actor / mailbox へ配送しない |
| system message delivery retransmission / nack | `SystemMessageDelivery.scala:51`, `SystemMessageDelivery.scala:83` | 部分実装 | std/association_runtime | medium | `SystemMessageDeliveryState` は sequence / cumulative ack まで。retransmission / nack は未実装 |
| reconnect / backoff runtime | `ArteryTcpTransport.scala:156` | 部分実装 | std/tcp_transport + std/association_runtime | medium | `Association::recover` はあるが、runtime が backoff / reconnect policy を駆動しない |

### 4. Wire protocol / serialization　✅ 実装済み 6/14 (43%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| Pekko Artery TCP framing | `artery/tcp/TcpFraming.scala:34` | 未対応 | std/tcp_transport | hard | fraktor は独自 `length + version + kind` frame。Pekko の `AKKA` magic / stream id / little-endian framing ではない |
| `ArteryMessageSerializer` control protocol | `serialization/ArteryMessageSerializer.scala:56` | 部分実装 | core/wire + actor-core/serialization | hard | `HandshakePdu` / `ControlPdu` / `AckPdu` はあるが、Pekko manifest / protobuf control 互換ではない |
| message payload serialization into envelope | `MessageSerializer.scala:81`, `ArteryMessageSerializer.scala:178` | 部分実装 | std/tcp_transport + actor-core/serialization | hard | `build_envelope_frame()` が `Bytes::new()` placeholder を使う |
| `MessageContainerSerializer` | `serialization/MessageContainerSerializer.scala:30` | 未対応 | actor-core/serialization | medium | actor selection message の remote payload 化がない |
| `SystemMessageSerializer` | `serialization/SystemMessageSerializer.scala:22` | 未対応 | actor-core/serialization | medium | Watch / Unwatch / DeathWatchNotification / Terminate の serializer がない |
| `MiscMessageSerializer` subset | `serialization/MiscMessageSerializer.scala:37` | 部分実装 | actor-core/serialization + core/wire | medium | `Address` / `UniqueAddress` はあるが、Identify / ActorIdentity / `RemoteRouterConfig` などの manifest 対応がない |
| `ThrowableNotSerializableException` | `serialization/ThrowableNotSerializableException.scala:22` | 対応済み (新名 `ThrowableNotSerializableError`) | actor-core/serialization | easy | 対応済み。Rust 慣習に合わせ `*Error` 命名 |
| Artery compression protocol (`CompressionProtocol`, `CompressionTable`, `InboundCompressions`, `TopHeavyHitters`) | `artery/compress/*`, `artery/Codecs.scala` | 未対応 | core/wire + core/association | hard | wire protocol 上の actor ref / manifest compression がごっそり未実装 |

### 5. Provider / remote actor ref / routing　✅ 実装済み 3/8 (38%)

`RemoteActorRef`, `RemoteActorRefProvider` trait, local/remote dispatch ruleを持つ `StdRemoteActorRefProvider` は存在するが、実体化と送信経路が未完了。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| concrete remote `ActorRef` construction | `RemoteActorRefProvider.scala:161`, `RemoteActorRefProvider.scala:673` | 部分実装 | std/provider | hard | remote branch は core resolve 後に `RemoteSenderBuildFailed` を返す |
| remote send path | `RemoteActorRefProvider.scala:763` | 部分実装 | std/provider + std/tcp_transport | hard | `RemoteActorRefSender` はあるが、payload serialization と watch integration が placeholder |
| remote DeathWatch interception | `RemoteActorRefProvider.scala:739` | 部分実装 | std/provider + std/watcher_actor + actor-core | hard | `watch` / `unwatch` intent はあるが `Terminated(AddressTerminated)` 統合がない |
| `RemoteRouterConfig` | `routing/RemoteRouterConfig.scala:47` | 未対応 | actor-core/routing + core/provider | medium | actor-core routing はあるが、remote node list に pool routee を展開する契約がない |
| remote deployment daemon / `useActorOnNode` | `RemoteDaemon.scala`, `RemoteDeployer.scala`, `RemoteDeploymentWatcher.scala` | 未対応 | std/provider | hard | remote child actor 作成要求と deployment watch の経路がない |

### 6. Watcher / DeathWatch runtime　✅ 実装済み 3/6 (50%)

`WatcherState`, `WatcherCommand`, `WatcherEffect`, `WatcherActor`, `run_heartbeat_loop` は存在する。watch bookkeeping と detector tick は動くが、actor-core への最終適用が不足する。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| heartbeat response protocol | `RemoteWatcher.scala:56`, `RemoteWatcher.scala:57` | 部分実装 | core/wire + std/watcher_actor | medium | heartbeat tick / receive はあるが、response uid / actor-system UID 検証は未接続 |
| watcher effects application | `RemoteWatcher.scala:103` | 部分実装 | std/watcher_actor + actor-core | hard | `WatcherActor` は `WatcherEffect` を `effect_tx` に流すだけで、`Terminated` / event stream 適用がない |
| `AddressTerminated` integration | `RemoteWatcher.scala:103`, `SystemMessageSerializer.scala:22` | 未対応 | actor-core + std/watcher_actor | hard | remote node failure を local DeathWatch へ統合する契約がない |

### 7. Instrumentation / config / logging　✅ 実装済み 5/6 (83%)

前回から改善が大きいカテゴリで、`RemoteLogMarker` と association effects の lifecycle publish は実装済みと判断できる。

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| advanced Artery settings subset | `ArterySettings.scala`, `RemoteSettings.scala` | 部分実装 | core/config | medium | `RemoteConfig` は canonical address / ack window / flight recorder を持つが、lanes / compression / restart backoff / watcher interval 等は不足 |

実装済みとして扱うもの: `RemotingLifecycleState`, `EventPublisher`, `ListenStarted` publish, `RemoteLogMarker`, `RemoteInstrument`, `RemotingFlightRecorder`。

## 対象外（n/a）

| Pekko API / 領域 | 判定理由 |
|------------------|----------|
| classic remoting `Endpoint*`, `AckedDelivery`, `PekkoProtocolTransport`, `transport/Transport.scala` | deprecated classic remoting |
| Netty transport / FailureInjectorTransportAdapter / ThrottlerTransportAdapter | deprecated classic transport または test/failure injection 用 |
| Aeron UDP transport | 特定実装技術の完全互換は固定スコープ外 |
| `SSLEngineProvider`, `ConfigSSLEngineProvider`, `RotatingKeysSSLEngineProvider` | Java `SSLEngine` 完全互換は対象外。Rust TLS adapter が必要なら別スコープ |
| JFR remoting flight recorder events | JVM Flight Recorder 固有 |
| HOCON provider loading / `FailureDetectorLoader` dynamic access | JVM classloader / reflection 固有 |
| Java serialization / Jackson module | serializer contract との接続点だけ対象 |

## 実装優先度

### Phase 2: medium

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `FailureDetectorRegistry[A]` / `DefaultFailureDetectorRegistry[A]` | core/failure_detector | カテゴリ2 |
| `RemoteTransport.start` の実 bind 契約 | std/tcp_transport + std/extension_installer | カテゴリ3 |
| handshake validation / retry / liveness probe | core/association + std/association_runtime | カテゴリ3 |
| per-peer inbound association routing | std/association_runtime | カテゴリ3 |
| system message delivery retransmission / nack | std/association_runtime | カテゴリ3 |
| reconnect / backoff runtime | std/tcp_transport + std/association_runtime | カテゴリ3 |
| `MessageContainerSerializer` | actor-core/serialization | カテゴリ4 |
| `SystemMessageSerializer` | actor-core/serialization | カテゴリ4 |
| `MiscMessageSerializer` subset | actor-core/serialization + core/wire | カテゴリ4 |
| advanced Artery settings subset | core/config | カテゴリ7 |

### Phase 3: hard

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| inbound envelope delivery | std/association_runtime + std/provider | カテゴリ3 |
| Pekko Artery TCP framing | std/tcp_transport | カテゴリ4 |
| `ArteryMessageSerializer` control protocol | core/wire + actor-core/serialization | カテゴリ4 |
| message payload serialization into envelope | std/tcp_transport + actor-core/serialization | カテゴリ4 |
| Artery compression protocol | core/wire + core/association | カテゴリ4 |
| concrete remote `ActorRef` construction | std/provider | カテゴリ5 |
| remote send path | std/provider + std/tcp_transport | カテゴリ5 |
| remote DeathWatch interception | std/provider + std/watcher_actor + actor-core | カテゴリ5 |
| `RemoteRouterConfig` | actor-core/routing + core/provider | カテゴリ5 |
| remote deployment daemon / `useActorOnNode` | std/provider | カテゴリ5 |
| watcher effects application | std/watcher_actor + actor-core | カテゴリ6 |
| `AddressTerminated` integration | actor-core + std/watcher_actor | カテゴリ6 |

## 内部モジュール構造ギャップ

今回は API / 実動作ギャップが支配的なため、内部モジュール構造ギャップの詳細分析は省略する。特に remote `ActorRef` 実体化、payload serialization、inbound envelope delivery、watcher effects の actor-core 適用が未完成であり、責務分割の最適化より先に公開契約と end-to-end 経路を閉じる段階である。

ただし、次版で構造分析へ進む場合の観点は以下になる。

| 構造観点 | 現状 | 次に見るべき点 |
|----------|------|----------------|
| `core::wire` と actor-core serialization の境界 | `wire` は独自 PDU を持つが byte payload が placeholder | `AnyMessage` → serialized bytes の責務をどこへ置くか |
| `association_runtime` の責務 | handshake / outbound / inbound / effect application が揃うが、peer registry と delivery adapter が未接続 | per-peer registry と delivery adapter を分けるか |
| provider と actor-core provider の境界 | local/remote dispatch まではあるが remote branch が `RemoteSenderBuildFailed` | `ActorSystemState` 依存をどこに閉じるか |
| watcher effect の適用先 | pure `WatcherState` と tokio actor はある | `Terminated` / event stream / system message への接続点 |

## まとめ

remote は `core` namespace への整理、address primitives、association state machine、`DeadlineFailureDetector`、address-bound `PhiAccrualFailureDetector`、`RemoteLogMarker`、`ListenStarted` publish まで進んでおり、前回レポートより確実に前進している。

低コストで parity を前進できるのは、failure detector registry、`RemoteTransport.start` の実 bind 契約、system message retransmission、serializer まわりの medium gap である。

主要ギャップは、remote `ActorRef` 実体化、payload serialization、Pekko Artery framing / control protocol、inbound envelope delivery、watcher effects / `AddressTerminated` の actor-core 統合である。したがって現時点では、内部構造の細部比較よりも、API surface と実配送経路の未完成部分を先に閉じるのが妥当である。
