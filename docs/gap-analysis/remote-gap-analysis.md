# remote モジュール ギャップ分析

更新日: 2026-04-24 (4th edition / 固定スコープ版)

## 比較スコープ定義

この調査は、Apache Pekko remote 全体を raw API 数で移植対象にするものではない。fraktor-rs の `remote` では、Pekko Artery compatible な remote actor transport 契約を対象にし、classic remoting / JVM 実装技術 / testkit は parity 分母から除外する。

### 対象に含めるもの

| 領域 | fraktor-rs | Pekko 参照 |
|------|------------|------------|
| remote core | `modules/remote-core/src/` | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/` |
| Artery transport contract | `modules/remote-core/src/transport/`, `modules/remote-core/src/association/`, `modules/remote-core/src/wire/` | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/artery/` |
| std TCP adapter | `modules/remote-adaptor-std/src/tcp_transport/`, `modules/remote-adaptor-std/src/association_runtime/` | `references/pekko/remote/src/main/scala/org/apache/pekko/remote/artery/tcp/` の TCP 契約 |
| remote actor ref provider | `modules/remote-core/src/provider/`, `modules/remote-adaptor-std/src/provider/` | `RemoteActorRefProvider.scala` / `RemoteActorRef` 相当 |
| failure detector / watcher | `modules/remote-core/src/failure_detector/`, `modules/remote-core/src/watcher/`, `modules/remote-adaptor-std/src/watcher_actor/` | `FailureDetector*.scala`, `RemoteWatcher.scala` |
| serialization 接続点 | `modules/actor-core/src/core/kernel/serialization/`, `modules/remote-core/src/wire/` | `remote/serialization/` の remote transport に必要な契約 |
| lifecycle / instrumentation | `modules/remote-core/src/extension/`, `modules/remote-core/src/instrument/` | `RemotingLifecycleEvent.scala`, `RemoteLogMarker.scala`, `RemoteInstrument.scala` |

### 対象から除外するもの

| 除外項目 | 理由 |
|----------|------|
| classic remoting / `Endpoint.scala` / `AckedDelivery.scala` | Pekko 側でも deprecated。Artery 互換の分母には入れない |
| Netty transport / Aeron UDP 完全互換 | JVM / Netty / Aeron 固有の transport 実装。Rust std TCP adapter とは別物 |
| TLS / `SSLEngineProvider` 完全互換 | Java `SSLEngine` / HOCON / classloader に依存する実装互換は除外。Rust TLS adapter が必要なら別スコープで扱う |
| Java serialization / Jackson module そのもの | serialization contract との接続点のみ対象 |
| HOCON provider loading / JVM dynamic access / classloader check | JVM 設定ロード方式に依存するため対象外 |
| JFR / Flight Recorder event class 完全互換 | JFR は JVM 固有。Rust 側は ring-buffer flight recorder で代替 |
| remote testkit / multi-node-testkit / remote-tests | runtime API ではない |

### raw 抽出値の扱い

`references/pekko/remote` の raw 抽出では public / private / deprecated / JVM 固有を含めて型宣言 579 件、主要 `def` 1594 件が見つかる。この数は参考値であり、固定スコープの parity カバレッジ分母には使わない。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 固定スコープ対象概念 | 約 54 |
| fraktor-rs 公開型数 | 69 (core: 50, std: 19) |
| fraktor-rs 公開メソッド数 | 239 (core: 177, std: 62) |
| 固定スコープ概念カバレッジ | 約 30/54 (56%) |
| hard gap | 6 |
| medium gap | 11 |
| easy gap | 8 |
| trivial gap | 1 |

remote は型の器と state machine はかなり揃っているが、end-to-end の remote actor delivery は未完成である。特に `StdRemoteActorRefProvider::actor_ref` の remote branch は `RemoteSenderBuildFailed` を返し、`TcpRemoteTransport::send` は payload を空 bytes で送る placeholder のままなので、公開型数だけではカバレッジを高く評価できない。

`todo!()` / `unimplemented!()` は検出されなかった。一方で、実動作上の placeholder / TODO コメントは残っているため、旧版の「スタブ0件」は「panic系スタブ0件、機能 placeholder あり」に修正する。

## 層別カバレッジ

| 層 | Pekko 対応範囲 | fraktor-rs 現状 | 評価 |
|----|----------------|-----------------|------|
| core / transport contract | `RemoteTransport`, address, association, handshake, quarantine | `RemoteTransport`, `Address`, `UniqueAddress`, `Association`, wire PDU は存在 | API surface は中程度。wire 互換と runtime 接続が不足 |
| core / provider | `RemoteActorRefProvider`, `RemoteActorRef`, actor ref resolution | `RemoteActorRefProvider`, `RemoteActorRef`, path resolver は存在 | remote `ActorRef` 生成と送信経路が未完 |
| core / failure detector | `FailureDetector`, `FailureDetectorRegistry`, `PhiAccrualFailureDetector` | `PhiAccrualFailureDetector`, `HeartbeatHistory` は実装済み | registry / address-aware SPI / deadline detector が不足 |
| std / TCP adapter | Artery TCP transport, framing, association runtime | tokio TCP server/client/frame codec はある | listener lifecycle、Artery framing、payload delivery が不足 |
| actor-core integration | serialization, event stream, DeathWatch | generic serialization/event stream はある | remote 固有 serializer と AddressTerminated 統合が不足 |

## カテゴリ別ギャップ

### 1. Address / identity　✅ 実装済み 4/4 (100%)

`Address`、`UniqueAddress`、`RemoteNodeId`、actor path からの remote address resolution は実装済み。Pekko の `UniqueAddress` と同じく `(Address, uid)` を保持する。

### 2. Failure detector　✅ 実装済み 3/6 (50%)

| Pekko API | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|-----------|------------|-----------------|----------|--------|------|
| `FailureDetectorRegistry[A]` / `DefaultFailureDetectorRegistry[A]` | `FailureDetectorRegistry.scala:30`, `DefaultFailureDetectorRegistry.scala:27` | 未対応 | core/failure_detector | medium | `WatcherState` 内部に detector map はあるが、汎用 registry API と reset/remove contract がない |
| `FailureDetectorWithAddress` | `FailureDetector.scala:43` | 未対応 | core/failure_detector | easy | `PhiAccrualFailureDetector` に monitored address を持たせる SPI がない。ログ/marker と組み合わせる前提 |
| `DeadlineFailureDetector` | `DeadlineFailureDetector.scala` | 未対応 | core/failure_detector | easy | Phi 以外の単純 deadline detector がない |

実装済み: `PhiAccrualFailureDetector`、`HeartbeatHistory`、Pekko の phi 計算式に近い logistic approximation。

### 3. Transport / association / lifecycle　✅ 実装済み 7/14 (50%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `RemoteTransport.start` が実 listener を起動する契約 | `RemoteTransport.scala:84`, `ArteryTcpTransport.scala:118` | 部分実装 | std/tcp_transport | medium | `StdRemoting::start` は同期 `start()` を呼ぶため `running = true` のみ。実 bind は `start_async()` を別途呼ぶ必要がある |
| advertised addresses / listen event | `RemoteTransport.scala:66`, `RemotingLifecycleEvent.scala:78` | 部分実装 | core/extension + std/extension_installer | easy | `TcpRemoteTransport::addresses` は実装済みだが `StdRemoting::addresses` は `&[]` を返す。`ListenStarted` variant はあるが emission が未接続 |
| handshake validation / retry / liveness probe | `Handshake.scala:37`, `Handshake.scala:63` | 部分実装 | core/association + std/association_runtime | medium | `run_inbound_dispatch` は peer string から `RemoteNodeId` を合成し、PDU の origin/to/uid 検証や retry/liveness probe がない |
| per-peer inbound association routing | `ArteryTransport.scala:280`, `Association.scala:1131` | 部分実装 | std/association_runtime | medium | inbound frame を渡された単一 `AssociationShared` に流すだけで、peer registry lookup がない |
| inbound envelope delivery | `ArteryTcpTransport.scala:405`, `MessageDispatcher.scala:33` | 部分実装 | std/association_runtime + std/provider | hard | `WireFrame::Envelope` は debug log のみで local actor へ配送されない |
| system message delivery retransmission / nack | `SystemMessageDelivery.scala:51`, `SystemMessageDelivery.scala:83` | 部分実装 | std/association_runtime | medium | `SystemMessageDeliveryState` は cumulative ack と pending queue のみ。timer retransmission / nack handling は TODO |
| reconnect / backoff runtime | `ArteryTcpTransport.scala:156` | 部分実装 | std/tcp_transport + std/association_runtime | medium | core に `gate` / `recover` はあるが、runtime 側で restart backoff を駆動する層が不足 |

### 4. Wire protocol / serialization　✅ 実装済み 6/14 (43%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| Pekko Artery TCP framing | `TcpFraming.scala:34`, `TcpFraming.scala:72` | 未対応 | std/tcp_transport | hard | fraktor は独自 big-endian `length + version + kind` frame。Pekko の `AKKA` magic / stream id / little-endian frame length とは非互換 |
| `ArteryMessageSerializer` control protocol | `ArteryMessageSerializer.scala:32`, `ArteryMessageSerializer.scala:56` | 部分実装 | core/wire + actor-core/serialization | hard | fraktor は `HandshakePdu` / `ControlPdu` / `AckPdu` を持つが、Pekko の manifest / protobuf control message と互換ではない |
| message payload serialization into envelope | `MessageSerializer.scala:81`, `ArteryMessageSerializer.scala:178` | 部分実装 | std/tcp_transport + actor-core/serialization | hard | `TcpRemoteTransport::build_envelope_frame` は payload に `Bytes::new()` を入れる placeholder |
| `MessageContainerSerializer` | `MessageContainerSerializer.scala:30` | 未対応 | actor-core/serialization | medium | actor selection message の remote payload 化がない |
| `SystemMessageSerializer` | `SystemMessageSerializer.scala:22` | 未対応 | actor-core/serialization | medium | Watch / Unwatch / DeathWatchNotification / Terminate などの system message serializer がない |
| `MiscMessageSerializer` subset | `MiscMessageSerializer.scala:37` | 部分実装 | actor-core/serialization + remote-core/wire | medium | `Address` / `UniqueAddress` / actor ref path など基礎型はあるが、Identify / ActorIdentity / RemoteScope / router config などの remote manifest 対応がない |
| `ThrowableNotSerializableException` | `ThrowableNotSerializableException.scala:22` | 未対応 | actor-core/serialization | trivial | 例外型相当の error payload を追加するだけでよい |
| Artery compression protocol | `CompressionProtocol.scala:22`, `CompressionTable.scala:30`, `TopHeavyHitters.scala:35` | 未対応 | core/wire + core/association | hard | Pekko では `private[remote]` だが wire protocol 上の actor ref / class manifest compression として必要 |

generic serialization registry、string manifest serializer、remote call scope は actor-core 側に存在する。ただし remote runtime からそれを使って実 payload を `EnvelopePdu` に詰める接続が未完。

### 5. Provider / remote actor ref / routing　✅ 実装済み 3/8 (38%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| concrete remote `ActorRef` construction | `RemoteActorRefProvider.scala:161`, `RemoteActorRefProvider.scala:673` | 部分実装 | std/provider | hard | `StdRemoteActorRefProvider::actor_ref` は remote branch 検証後に `RemoteSenderBuildFailed` を返す |
| remote send path | `RemoteActorRefProvider.scala:763` | 部分実装 | std/provider + std/tcp_transport | hard | `RemoteActorRefSender` はあるが、payload serialization と watcher integration が placeholder |
| remote DeathWatch interception | `RemoteActorRefProvider.scala:739` | 部分実装 | std/provider + std/watcher_actor + actor-core | hard | watch/unwatch API はあるが、system message path と `Terminated(AddressTerminated)` 通知の統合がない |
| `RemoteRouterConfig` | `routing/RemoteRouterConfig.scala:47` | 未対応 | actor-core/routing + remote-core/provider | medium | actor-core routing はあるが、remote node list に pool routee を展開する config がない |
| remote deployment daemon / `useActorOnNode` | `RemoteActorRefProvider.scala:596` | 未対応 | std/provider | medium | JVM classpath 依存部分は除外するが、remote node へ child actor 作成要求を送る契約は未対応 |

### 6. Watcher / DeathWatch runtime　✅ 実装済み 3/6 (50%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| heartbeat response protocol | `RemoteWatcher.scala:56`, `RemoteWatcher.scala:57` | 部分実装 | core/wire + std/watcher_actor | medium | `ControlPdu::Heartbeat` はあるが response uid と actor-system UID 検証が未接続 |
| watcher effects application | `RemoteWatcher.scala:103` | 部分実装 | std/watcher_actor + actor-core | hard | `WatcherEffect::NotifyTerminated` / `NotifyQuarantined` は生成されるが、actor-core へ配送する実 adapter がない |
| AddressTerminated integration | `RemoteWatcher.scala:103`, `SystemMessageSerializer.scala:22` | 未対応 | actor-core + std/watcher_actor | hard | remote node failure を local watchers の `Terminated` / address terminated として届ける契約が不足 |

### 7. Instrumentation / config / logging　✅ 実装済み 3/6 (50%)

| Pekko API / 契約 | Pekko 参照 | fraktor-rs 対応 | 実装先層 | 難易度 | 備考 |
|------------------|------------|-----------------|----------|--------|------|
| `RemoteLogMarker` | `RemoteLogMarker.scala:27` | 未対応 | core/instrument or actor-core/logging | easy | `failureDetectorGrowing`, `quarantine`, `connect`, `disconnected` 相当の marker がない |
| advanced Artery settings subset | `ArterySettings.scala`, `RemoteSettings.scala` | 部分実装 | core/config | medium | `RemoteConfig` は canonical/timeout/ack/flight recorder を持つが、stream lanes / compression / restart backoff / watcher interval 等が不足 |
| lifecycle event publishing from association effects | `RemotingLifecycleEvent.scala:138` | 部分実装 | std/association_runtime + core/extension | easy | `EventPublisher` はあるが `apply_effects_in_place` は TODO のまま tracing log に留まる |

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

### Phase 1: trivial / easy

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `ThrowableNotSerializableException` 相当 | actor-core/serialization | カテゴリ4 |
| `FailureDetectorWithAddress` | core/failure_detector | カテゴリ2 |
| `DeadlineFailureDetector` | core/failure_detector | カテゴリ2 |
| advertised addresses / listen event | core/extension + std/extension_installer | カテゴリ3 |
| `RemoteLogMarker` | core/instrument or actor-core/logging | カテゴリ7 |
| lifecycle event publishing from association effects | std/association_runtime + core/extension | カテゴリ7 |

### Phase 2: medium

| 項目 | 実装先層 | 根拠 |
|------|----------|------|
| `FailureDetectorRegistry[A]` / `DefaultFailureDetectorRegistry[A]` | core/failure_detector | カテゴリ2 |
| `RemoteTransport.start` が実 listener を起動する契約 | std/tcp_transport | カテゴリ3 |
| handshake validation / retry / liveness probe | core/association + std/association_runtime | カテゴリ3 |
| per-peer inbound association routing | std/association_runtime | カテゴリ3 |
| system message delivery retransmission / nack | std/association_runtime | カテゴリ3 |
| reconnect / backoff runtime | std/tcp_transport + std/association_runtime | カテゴリ3 |
| `MessageContainerSerializer` | actor-core/serialization | カテゴリ4 |
| `SystemMessageSerializer` | actor-core/serialization | カテゴリ4 |
| `MiscMessageSerializer` subset | actor-core/serialization + remote-core/wire | カテゴリ4 |
| `RemoteRouterConfig` | actor-core/routing + remote-core/provider | カテゴリ5 |
| remote deployment daemon / `useActorOnNode` | std/provider | カテゴリ5 |
| heartbeat response protocol | core/wire + std/watcher_actor | カテゴリ6 |
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
| watcher effects application | std/watcher_actor + actor-core | カテゴリ6 |
| AddressTerminated integration | actor-core + std/watcher_actor | カテゴリ6 |

## 内部モジュール構造ギャップ

今回は API / 実動作ギャップが支配的なため、内部モジュール構造ギャップの詳細分析は省略する。特に remote `ActorRef` 生成、payload serialization、inbound delivery、DeathWatch integration が未完成なので、責務分割の最適化より先に公開契約と end-to-end 経路を閉じるべき段階である。

ただし、次版で構造分析に進む場合の観点は以下になる。

| 構造観点 | 現状 | 次に見るべき点 |
|----------|------|----------------|
| `wire` と actor-core serialization の境界 | `wire` は独自 PDU、actor-core は generic registry | remote runtime がどこで `AnyMessage` を `SerializedMessage` に変換するか |
| `association_runtime` の責務 | handshake / inbound dispatch / effect application が Phase B placeholder を含む | per-peer registry、event publish、delivery adapter を分けるか |
| provider と actor-core provider の境界 | remote branch が `RemoteSenderBuildFailed` | `ActorSystemState` 依存をどこに閉じるか |
| watcher effect の適用先 | pure `WatcherState` と tokio actor はある | actor-core `Terminated` / event stream / system message への接続点 |

## まとめ

remote は `Address`、`UniqueAddress`、association state machine、Phi accrual failure detector、tokio TCP skeleton、wire PDU などの基礎部品は揃っている。一方で、Pekko Artery compatible な remote actor transport として見ると、end-to-end delivery、wire/serialization、DeathWatch、provider integration がまだ大きい。

低コストで parity を前進できるのは、`StdRemoting::addresses` と listen event、`FailureDetectorWithAddress`、`RemoteLogMarker`、`ThrowableNotSerializableException` などの Phase 1 項目。主要ギャップは、remote `ActorRef` の実生成、payload serialization、Pekko Artery framing/control protocol、inbound delivery、AddressTerminated 統合である。

したがって現時点では、内部構造の細部比較よりも、API surface と実配送経路の未完成部分を先に閉じるのが妥当である。
