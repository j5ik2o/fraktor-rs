# remote モジュール ギャップ分析

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数 | 78（JFRイベント40型・private[remote]内部型を除外） |
| fraktor-rs 公開型数 | 72（core: 66, std: 6） |
| カバレッジ（型単位） | 52/78 (67%) |
| ギャップ表の行数 | 61（core: 38, std: 6, core+std: 1, n/a: 16） |

> **注記**: Pekkoの Classic Transport（deprecated）12型と Aeron UDP 4型は対象外（n/a）として計上。
> 実質的な比較対象は62型であり、その基準では 52/62 = **84%** のカバレッジ。
> また、ギャップ表は `RemotingLifecycleEvent` などの sealed hierarchy を個別行へ分解しているため、
> 上記の「ギャップ表の行数」は型単位カバレッジの差分と 1:1 では対応しない。

## 層別カバレッジ

| 層 | Pekko対応数 | fraktor-rs実装数 | カバレッジ |
|----|-------------|------------------|-----------|
| core（コアロジック・trait） | 55 | 66 | 120%（fraktor-rsの方が細粒度設計） |
| std（アダプタ） | 7 | 6 | 86% |
| n/a（JVM固有・deprecated） | 16 | — | — |

## カテゴリ別ギャップ

---

### 1. コアリモーティング基盤　✅ 実装済み 6/10 (60%)

fraktor-rs 対応:
- `RemoteActorRefProvider` → `RemoteActorRefProvider` ✅
- `RemoteTransport` (abstract) → `RemoteTransport` trait ✅
- `RemoteTransportException` → `RemotingError` ✅
- `RemoteSettings` → `RemotingExtensionConfig` ✅
- `RARP` Extension → `RemotingExtension` + `RemotingExtensionId` ✅
- `Remoting` lifecycle → `RemotingControl` + `RemotingControlHandle` ✅

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RemoteDaemon` | `RemoteDaemon.scala` | 未対応 | core | medium | リモートアクター生成を受け付けるデーモンアクター。remote deploy機能に必要 |
| `RemoteDeployer` | `RemoteDeployer.scala` | 未対応 | core | medium | リモートノードへの Props デプロイ。RemoteDaemon と対 |
| `RemoteScope` | `RemoteDeployer.scala:L28` | 未対応 | core | easy | デプロイ先ノードを指定する case class。`Address` のラッパー |
| `AddressUidExtension` | `AddressUidExtension.scala` | 未対応 | core | easy | `RemoteNodeId` と同じく、Address + UID の識別責務自体は adapter ではなく core に属する |

---

### 2. 障害検出　✅ 実装済み 6/7 (86%)

fraktor-rs 対応:
- `FailureDetector` trait → `FailureDetector` trait ✅
- `FailureDetectorRegistry[A]` → `FailureDetectorRegistry<A>` ✅
- `DefaultFailureDetectorRegistry` → `DefaultFailureDetectorRegistry<A>` ✅
- `PhiAccrualFailureDetector` → `PhiFailureDetector` ✅
- `DeadlineFailureDetector` → `DeadlineFailureDetector` ✅
- （追加）`PhiFailureDetectorConfig`, `DeadlineFailureDetectorConfig` ✅

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `FailureDetectorWithAddress` | `FailureDetector.scala:L43` | 未対応 | core | trivial | `FailureDetector` + `address(): Address` を追加するだけのtrait |

---

### 3. Artery トランスポート　✅ 実装済み 18/24 (75%)

fraktor-rs 対応:
- `ArteryTransport` → `RemoteTransport` trait ✅
- `ArteryTcpTransport` → `TokioTcpTransport` (std) ✅
- `Association` / `AssociationState` → `EndpointAssociationCoordinator` + `AssociationState` ✅
- `InboundEnvelope` → `InboundEnvelope` ✅
- `OutboundEnvelope` → `RemotingEnvelope` ✅
- `SystemMessageDelivery` → `SystemMessageEnvelope` + `AckedDelivery` ✅
- `Control.Flush` → `Flush` ✅
- `Control.FlushAck` → `FlushAck` ✅
- `Handshake` → `HandshakeFrame` + `HandshakeKind` ✅
- `SendQueue` → `EndpointWriter` ✅
- `MessageDispatcher` → `EndpointReader` ✅
- 各種 Transport 抽象（TransportBind, TransportChannel, TransportEndpoint, TransportHandle） ✅
- `LoopbackTransport` → `LoopbackTransport` ✅

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Codecs` (Encoder/Decoder) | `Codecs.scala` | 未対応 | core | medium | シリアライズ/デシリアライズのパイプラインステージ。圧縮テーブル管理を含む |
| `EnvelopeBufferPool` | `EnvelopeBufferPool.scala` | 未対応 | core | medium | メッセージバッファのオブジェクトプール。パフォーマンス最適化 |
| `RestartCounter` | `RestartCounter.scala` | 未対応 | core | trivial | 時間ウィンドウ内のリスタート回数を追跡。数十行の実装 |
| `InboundQuarantineCheck` | `InboundQuarantineCheck.scala` | 未対応 | core | easy | 受信メッセージの quarantine チェックステージ |
| `FlushBeforeDeathWatchNotification` | `FlushBeforeDeathWatchNotification.scala` | 未対応 | core | easy | DeathWatch 通知前にフラッシュを保証するステージ |
| `FlushOnShutdown` | `FlushOnShutdown.scala` | 未対応 | core | easy | シャットダウン前にフラッシュを保証するステージ |

---

### 4. SSL/TLS セキュリティ　✅ 実装済み 0/8 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `SSLEngineProvider` (artery/tcp) | `artery/tcp/SSLEngineProvider.scala:L24` | 未対応 | core (trait) + std (impl) | medium | TLS接続のSSLEngine生成SPI |
| `ConfigSSLEngineProvider` | `artery/tcp/ConfigSSLEngineProvider.scala` | 未対応 | std | medium | 設定ファイルベースのSSL実装 |
| `RotatingKeysSSLEngineProvider` | `artery/tcp/ssl/RotatingKeysSSLEngineProvider.scala` | 未対応 | std | hard | PEMファイルのホットリロード対応SSL |
| `SSLEngineProviderSetup` | `artery/tcp/SSLEngineProvider.scala:L75` | 未対応 | std | easy | プログラマティックSSL設定 |
| `SslTransportException` | `artery/tcp/SSLEngineProvider.scala:L46` | 未対応 | core | trivial | SSL例外型 |
| `SessionVerifier` | `artery/tcp/ssl/SessionVerifier.scala` | 未対応 | core (trait) | easy | TLSセッション検証SPI |
| `PemManagersProvider` | `artery/tcp/ssl/PemManagersProvider.scala` | 未対応 | std | medium | PEM証明書からKeyManager/TrustManager生成 |
| `SecureRandomFactory` | `security/provider/SeedSize.scala` | 未対応 | std | easy | セキュアランダム生成ファクトリ |

---

### 5. 圧縮　✅ 実装済み 0/5 (0%) — 全て private[remote]

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `CompressionProtocol` | `compress/CompressionProtocol.scala` | 未対応 | core | hard | ActorRef/ClassManifest 圧縮プロトコル。広告・ACKの双方向プロトコル |
| `CompressionTable` | `compress/CompressionTable.scala` | 未対応 | core | medium | 圧縮テーブル（key→id マッピング） |
| `DecompressionTable` | `compress/DecompressionTable.scala` | 未対応 | core | medium | 展開テーブル（id→value マッピング） |
| `InboundCompressions` | `compress/InboundCompressions.scala` | 未対応 | core | hard | 受信側圧縮管理。テーブル切り替え・広告送信を統合 |
| `TopHeavyHitters` | `compress/TopHeavyHitters.scala` | 未対応 | core | medium | ヒープ+ハッシュマップの頻出要素追跡データ構造 |

---

### 6. シリアライゼーション　✅ 実装済み 1/8 (13%)

fraktor-rs 対応:
- ワイヤフォーマット → `WireError` ✅（プロトコル違反エラー）

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `MessageSerializer` | `MessageSerializer.scala` | 未対応 | core | medium | メッセージの汎用シリアライズ/デシリアライズ |
| `ProtobufSerializer` | `serialization/ProtobufSerializer.scala` | 未対応 | core | medium | ワイヤフォーマット / envelope 変換の責務であり、`std` transport adapter ではなく core に置くべき |
| `SystemMessageSerializer` | `serialization/SystemMessageSerializer.scala` | 未対応 | core | medium | システムメッセージの wire 変換であり adapter 依存ではない |
| `MiscMessageSerializer` | `serialization/MiscMessageSerializer.scala` | 未対応 | core | easy | 汎用メッセージの wire 変換。`modules/remote/src/std` の責務ではない |
| `MessageContainerSerializer` | `serialization/MessageContainerSerializer.scala` | 未対応 | core | easy | ActorSelectionMessage の wire 変換であり core 配置が妥当 |
| `ThrowableNotSerializableException` | `serialization/ThrowableNotSerializableException.scala` | 未対応 | core | trivial | シリアライズ不可例外のラッパー |
| `ActorRefResolveCache` | `serialization/ActorRefResolveCache.scala` | 未対応 | core | easy | ActorRef パス解決のLRUキャッシュ |

---

### 7. ライフサイクルイベント　✅ 実装済み 3/11 (27%)

fraktor-rs 対応:
- `EventPublisher` → `EventPublisher` ✅
- `QuarantinedEvent` → `QuarantineReason` ✅（概念的対応）
- Backpressure イベント → `RemotingBackpressureListener` ✅

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RemotingLifecycleEvent` (sealed) | `RemotingLifecycleEvent.scala:L26` | 未対応 | core | easy | ライフサイクルイベントの sealed trait 階層 → Rust enum |
| `AssociatedEvent` | `RemotingLifecycleEvent.scala:L46` | 未対応 | core | trivial | 接続確立イベント |
| `DisassociatedEvent` | `RemotingLifecycleEvent.scala:L56` | 未対応 | core | trivial | 接続切断イベント |
| `AssociationErrorEvent` | `RemotingLifecycleEvent.scala:L64` | 未対応 | core | trivial | 接続エラーイベント |
| `RemotingListenEvent` | `RemotingLifecycleEvent.scala:L78` | 未対応 | core | trivial | リスニング開始イベント |
| `RemotingShutdownEvent` | `RemotingLifecycleEvent.scala:L96` | 未対応 | core | trivial | シャットダウンイベント |
| `RemotingErrorEvent` | `RemotingLifecycleEvent.scala:L96` | 未対応 | core | trivial | リモーティングエラーイベント |
| `GracefulShutdownQuarantinedEvent` | `RemotingLifecycleEvent.scala:L118` | 未対応 | core | trivial | グレースフルシャットダウン時の quarantine |

---

### 8. リモートウォッチャー　✅ 実装済み 4/5 (80%)

fraktor-rs 対応:
- `RemoteWatcher` → `watcher/` モジュール（`WatcherDaemon`） ✅
- `Heartbeat` → `Heartbeat` ✅
- `HeartbeatRsp` → `HeartbeatRsp` ✅
- `WatchRemote` / `UnwatchRemote` → `watcher/command` ✅

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RemoteDeploymentWatcher` | `RemoteDeploymentWatcher.scala` | 未対応 | core | medium | リモートデプロイされたアクターの監視。RemoteDeployer と対 |

---

### 9. フライトレコーダー　✅ 実装済み 4/4 (100%)

fraktor-rs 対応:
- `RemotingFlightRecorder` → `RemotingFlightRecorder` ✅
- JFR 具体イベント → `FlightMetricKind` enum + `RemotingMetric` ✅
- スナップショット → `RemotingFlightRecorderSnapshot` ✅
- Extension → `RemotingExtension` 経由 ✅

（ギャップなし）

---

### 10. リモートインストゥルメント　✅ 実装済み 2/2 (100%)

fraktor-rs 対応:
- `RemoteInstrument` → `RemoteInstrument` trait ✅
- インストゥルメント管理 → `RemoteInstruments` ✅

（ギャップなし）

---

### 11. バックプレッシャー　✅ 実装済み 3/3 (100%) — fraktor-rs 独自設計

fraktor-rs 対応:
- `RemotingBackpressureListener` trait ✅
- `FnRemotingBackpressureListener` ✅
- `TransportBackpressureHook` trait ✅
- `TransportBackpressureHookShared` ✅

（ギャップなし — Pekkoでは Artery Streams の背圧に組み込み。fraktor-rsは明示的 trait で設計）

---

### 12. ユーティリティ　✅ 実装済み 3/7 (43%)

fraktor-rs 対応:
- `UniqueAddress` → `RemoteNodeId` ✅（概念的対応）
- `AckedDelivery` → `AckedDelivery` enum ✅
- バウンドアドレス情報 → `RemoteAuthoritySnapshot` ✅

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `UniqueAddress` | `UniqueAddress.scala` | 別名で実装済み | — | — | `RemoteNodeId` が同等の責務を持つ（Address + UID） |
| `BoundAddressesExtension` | `BoundAddressesExtension.scala` | 未対応 | std | easy | バインド済みアドレスを公開する Extension |
| `LruBoundedCache` | `artery/LruBoundedCache.scala` | 未対応 | core | medium | LRUキャッシュ（ActorRef解決等で使用） |
| `ObjectPool` | `artery/ObjectPool.scala` | 未対応 | core | medium | ロックフリーオブジェクトプール |
| `ImmutableLongMap` | `artery/ImmutableLongMap.scala` | 未対応 | core | easy | 不変Long→Valueマップ |

---

### 13. ルーティング　✅ 実装済み 0/1 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RemoteRouterConfig` | `routing/RemoteRouterConfig.scala` | 未対応 | core | medium | リモートノード群にルーターをデプロイする Pool 設定 |

---

### 14. Classic Transport（deprecated）　対象外 0/12 (n/a)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `Transport` trait | `transport/Transport.scala` | n/a | — | — | deprecated: Artery に置き換え済み |
| `AssociationHandle` | `transport/Transport.scala:L233` | n/a | — | — | deprecated |
| `PekkoProtocolTransport` | `transport/PekkoProtocolTransport.scala` | n/a | — | — | deprecated |
| `PekkoPduCodec` | `transport/PekkoPduCodec.scala` | n/a | — | — | deprecated |
| `AbstractTransportAdapter` | `transport/AbstractTransportAdapter.scala` | n/a | — | — | deprecated |
| `ThrottlerTransportAdapter` | `transport/ThrottlerTransportAdapter.scala` | n/a | — | — | deprecated。テスト用にも使われる |
| `FailureInjectorTransportAdapter` | `transport/FailureInjectorTransportAdapter.scala` | n/a | — | — | deprecated。テスト専用 |
| `NettyTransport` | `transport/netty/NettyTransport.scala` | n/a | — | — | deprecated: Artery TCP に置き換え |
| `TestTransport` | `transport/TestTransport.scala` | n/a | — | — | deprecated |
| `TransportAdapterProvider` | `transport/AbstractTransportAdapter.scala:L32` | n/a | — | — | deprecated |
| `SchemeAugmenter` | `transport/AbstractTransportAdapter.scala:L70` | n/a | — | — | deprecated |
| `HandshakeInfo` (classic) | `transport/PekkoProtocolTransport.scala:L96` | n/a | — | — | deprecated |

---

### 15. Aeron UDP トランスポート　対象外 0/4 (n/a)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `ArteryAeronUdpTransport` | `artery/aeron/ArteryAeronUdpTransport.scala` | n/a | — | — | Aeron media driver 依存。JVM固有 |
| `AeronSink` | `artery/aeron/AeronSink.scala` | n/a | — | — | JVM固有 |
| `AeronSource` | `artery/aeron/AeronSource.scala` | n/a | — | — | JVM固有 |
| `TaskRunner` | `artery/aeron/TaskRunner.scala` | n/a | — | — | JVM固有 |

---

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）

- `FailureDetectorWithAddress` — core。`FailureDetector` trait にメソッド追加で対応可能
- `RestartCounter` — core。時間ウィンドウ内のカウント追跡。20-30行
- `SslTransportException` — core。エラー型の追加
- `ThrowableNotSerializableException` — core。エラー型の追加
- `RemotingLifecycleEvent` 階層 — core。Rust enum で `AssociatedEvent`, `DisassociatedEvent`, `RemotingListenEvent` 等を定義
- `RemoteScope` — core。`Address` のnewtypeラッパー

### Phase 2: easy（単純な新規実装）

- `AddressUidExtension` — core。Address + UID の識別責務
- `BoundAddressesExtension` — std。バインドアドレス公開
- `InboundQuarantineCheck` — core。quarantine チェックロジック
- `FlushBeforeDeathWatchNotification` — core。DeathWatch前フラッシュ
- `FlushOnShutdown` — core。シャットダウン前フラッシュ
- `ImmutableLongMap` — core。データ構造
- `SessionVerifier` — core。TLSセッション検証trait
- `ActorRefResolveCache` — core。LRUキャッシュでActorRefパス解決を高速化
- `SSLEngineProviderSetup` — std。SSL設定のプログラマティック提供
- `MiscMessageSerializer` — core
- `MessageContainerSerializer` — core

### Phase 3: medium（中程度の実装工数）

- `RemoteDaemon` — core。リモートアクター生成デーモン。actor モジュールとの連携が必要
- `RemoteDeployer` — core。リモートデプロイ機構。RemoteDaemonと対
- `RemoteDeploymentWatcher` — core。リモートデプロイされたアクター監視
- `Codecs` (Encoder/Decoder) — core。シリアライズパイプライン
- `EnvelopeBufferPool` / `ObjectPool` — core。パフォーマンス最適化のオブジェクトプール
- `LruBoundedCache` — core。汎用LRUキャッシュ
- `MessageSerializer` — core。汎用メッセージシリアライズ
- `ProtobufSerializer` — core。Protocol Buffers ベースの wire 変換
- `SystemMessageSerializer` — core。システムメッセージの wire 変換
- `SSLEngineProvider` — core (trait) + std (impl)。TLS接続のSPI
- `ConfigSSLEngineProvider` — std。設定ベースSSL実装
- `CompressionTable` / `DecompressionTable` — core。圧縮テーブルデータ構造
- `RemoteRouterConfig` — core。リモートルーター設定
- `TopHeavyHitters` — core。頻出要素追跡

### Phase 4: hard（アーキテクチャ変更を伴う）

- `CompressionProtocol` — core。双方向圧縮広告プロトコル。Codecs と連携必須
- `InboundCompressions` — core。受信圧縮管理。CompressionTable の切り替え・テーブルバージョニング
- `RotatingKeysSSLEngineProvider` — std。PEMファイルのホットリロード。ファイルウォッチ機構が必要
- `PemManagersProvider` — std。PEM証明書パース。rustls/native-tls との連携

### 対象外（n/a）

- **Classic Transport** 全12型 — deprecated。fraktor-rsは最初から Artery 相当のモダン設計
- **Aeron UDP** 全4型 — JVM Aeron media driver 固有。Rust では不要
- **JFR Events** 約40型 — Java Flight Recorder 固有。fraktor-rsは `RemotingFlightRecorder` + `FlightMetricKind` で抽象化済み

---

## まとめ

**全体カバレッジ**: 主要な基盤機能（トランスポート、障害検出、ウォッチャー、ハンドシェイク、エンドポイント管理、フライトレコーダー）は **十分にカバーされている**。deprecated/JVM固有の型を除くと実質84%のカバレッジ。

**即座に価値を提供できる未実装機能（Phase 1〜2）**:
- `RemotingLifecycleEvent` enum — イベント駆動の監視・ロギングに不可欠。`EventPublisher` は存在するがイベント型定義が不足
- `RestartCounter` — リスタート制御に必要な小さなユーティリティ
- `FlushBeforeDeathWatchNotification` / `FlushOnShutdown` — メッセージ順序保証の信頼性向上

**実用上の主要ギャップ（Phase 3〜4）**:
- **シリアライゼーション層** — `MessageSerializer` / `ProtobufSerializer` 等が未整備。ノード間通信の相互運用性に影響
- **SSL/TLS サポート** — セキュア通信が未対応。本番環境での利用に必須
- **圧縮プロトコル** — 高スループット環境でのパフォーマンス最適化
- **リモートデプロイ** — `RemoteDaemon` / `RemoteDeployer` がないとリモートノードへのアクター配置ができない

**YAGNI観点での省略推奨**:
- Classic Transport（12型）— deprecated であり実装不要
- Aeron UDP Transport（4型）— JVM固有のメディアドライバー。Rust では QUIC 等の代替を検討すべき
- JFR Events（40型）— Java Flight Recorder 固有。`FlightMetricKind` enum による抽象化で十分
- `ThrottlerTransportAdapter` / `FailureInjectorTransportAdapter` — テスト専用。`LoopbackTransport` で代替可能
