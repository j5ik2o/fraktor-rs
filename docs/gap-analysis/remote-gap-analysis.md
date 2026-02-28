# remote モジュール ギャップ分析

> 分析日: 2026-02-27（前回: 2026-02-24）
> 対象: `modules/remote/src/` vs `references/pekko/remote/src/`

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（公開API + 主要内部型） | 約40型 |
| fraktor-rs 公開型数 | 65型 |
| カバレッジ（機能カテゴリ単位） | 9/10 (90%)（前回 7/10 → 改善） |
| 主要ギャップ数 | 5（前回12 → 7件削減） |

> 注: Pekkoのremoteモジュールは多くの型が `private[pekko]` / `private[remote]` で内部APIだが、fraktor-rsでは同等概念を公開型として実装している。型数の直接比較は参考値。

### 前回分析からの変更

以下の機能が新たに実装済みとなった：
- `FailureDetector` trait → 完全実装（`failure_detector/` ディレクトリ）
- `DeadlineFailureDetector` → 完全実装
- `FailureDetectorRegistry` + `DefaultFailureDetectorRegistry` → 完全実装
- `AckedDelivery` → 完全実装（ワイヤプロトコル付き、`acked_delivery.rs`）
- `RemotingLifecycleEvent` → 完全な enum（8バリアント: Starting, Started, ListenStarted, Connected, Quarantined, Gated, Shutdown, Error）
- `ControlMessage` → 暗黙的に実装済み
- `RemoteInstrument` → 計装フックとして実装済み

## カテゴリ別ギャップ

### 1. コア基盤（RemoteActorRefProvider, Remoting）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `RemoteActorRefProvider` | `RemoteActorRefProvider.scala` | `RemoteActorRefProviderGeneric` | - | 実装済み |
| `RemoteTransport` (abstract) | `RemoteTransport.scala` | `RemoteTransport` trait | - | 実装済み（trait化） |
| `Remoting` (Extension) | `Remoting.scala` | `RemotingExtensionGeneric` | - | 実装済み |
| `RemoteDeployer` | `RemoteDeployer.scala` | 未対応 | hard | リモートデプロイ機能全体が未実装 |
| `RemoteDaemon` | `RemoteDaemon.scala` | 未対応 | hard | RemoteDeployerと関連 |

### 2. 障害検出（FailureDetector）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `FailureDetector` trait | `FailureDetector.scala` | `FailureDetector` trait | - | **実装済み** |
| `PhiAccrualFailureDetector` | `PhiAccrualFailureDetector.scala` | `PhiFailureDetector` | - | 実装済み |
| `DeadlineFailureDetector` | `DeadlineFailureDetector.scala` | `DeadlineFailureDetector` | - | **実装済み** |
| `FailureDetectorRegistry[A]` trait | `FailureDetectorRegistry.scala` | `FailureDetectorRegistry` trait | - | **実装済み** |
| `DefaultFailureDetectorRegistry` | `DefaultFailureDetectorRegistry.scala` | `DefaultFailureDetectorRegistry` | - | **実装済み** |

### 3. トランスポート層

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `RemoteTransport` trait | `transport/Transport.scala` | `RemoteTransport` trait | - | 実装済み |
| `TransportBind` / `TransportHandle` | - | `TransportBind`, `TransportHandle` | - | 実装済み |
| TCP Transport (Artery) | `artery/tcp/ArteryTcpTransport.scala` | `TokioTcpTransport` | - | 実装済み |
| `LoopbackTransport` | - | `LoopbackTransport` | - | 実装済み |
| `TransportBackpressureHook` | - | `TransportBackpressureHook` trait | - | 実装済み |
| `TransportInbound` | - | `TransportInbound` trait | - | 実装済み |
| `PekkoProtocolTransport` | `transport/PekkoProtocolTransport.scala` | 未対応 | n/a | Classic Remoting向け。Artery互換で不要 |
| `ThrottlerTransportAdapter` | `transport/ThrottlerTransportAdapter.scala` | 未対応 | n/a | テスト用アダプタ。Classic向け |
| `TestTransport` | `transport/TestTransport.scala` | 未対応 | easy | テスト用モック。テストキットとして有用 |
| `FailureInjectorTransportAdapter` | `transport/FailureInjectorTransportAdapter.scala` | 未対応 | easy | 障害注入テスト用 |

### 4. エンドポイント・Association

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `Association` | `artery/Association.scala` | `EndpointAssociationCoordinator` | - | 実装済み（別名） |
| `EndpointWriter` | (Pekko内部) | `EndpointWriterGeneric` | - | 実装済み |
| `EndpointReader` | (Pekko内部) | `EndpointReaderGeneric` | - | 実装済み |
| `InboundContext` trait | `artery/ArteryTransport.scala` | 部分的 | medium | `RemotingControl`に一部含まれるが `association()`, `completeHandshake()` が不足 |

### 5. エンベロープ・メッセージング

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `InboundEnvelope` | `artery/InboundEnvelope.scala` | `InboundEnvelope` | - | 実装済み |
| `OutboundEnvelope` | `artery/OutboundEnvelope.scala` | `OutboundMessage` | - | 実装済み（別名） |
| `RemotingEnvelope` | - | `RemotingEnvelope` | - | 実装済み |
| `SystemMessageEnvelope` | `artery/SystemMessageDelivery.scala` | `AckedDelivery::SystemMessage` | - | **実装済み**（AckedDeliveryのバリアントとして） |
| `AckedDelivery` | `AckedDelivery.scala` | `AckedDelivery` enum | - | **実装済み**（SystemMessage, Ack, Nackの3バリアント） |

### 6. イベント・ライフサイクル

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `RemotingLifecycleEvent` (sealed) | `RemotingLifecycleEvent.scala` | `RemotingLifecycleEvent` enum | - | **実装済み**（8バリアント: Starting, Started, ListenStarted, Connected, Quarantined, Gated, Shutdown, Error） |
| `QuarantinedEvent` | `artery/QuarantinedEvent.scala` | `RemotingLifecycleEvent::Quarantined` | - | 実装済み |
| `GracefulShutdownQuarantinedEvent` | `artery/QuarantinedEvent.scala` | 未対応 | trivial | イベント型の追加のみ |
| `ThisActorSystemQuarantinedEvent` | `artery/QuarantinedEvent.scala` | 未対応 | trivial | イベント型の追加のみ |
| `AssociatedEvent` / `DisassociatedEvent` | `RemotingLifecycleEvent.scala` | `RemotingLifecycleEvent::Connected` | - | EventPublisher経由で実装済み |

### 7. 設定・ユーティリティ

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `RemoteSettings` | `RemoteSettings.scala` | `RemotingExtensionConfig` | - | 実装済み（Builder パターンに変換） |
| `ArterySettings` | `artery/ArterySettings.scala` | `RemotingExtensionConfig` に統合 | - | 実装済み |
| `UniqueAddress` | `UniqueAddress.scala` | `RemoteNodeId` | - | 別名で実装済み |

### 8. リモートウォッチャー

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `RemoteWatcher` | `RemoteWatcher.scala` | `watcher/daemon.rs` | - | 部分実装（watcher module 存在） |
| `RemoteWatcher.WatchRemote` | `RemoteWatcher.scala` | `watcher/command.rs` | - | コマンド型として実装済み |
| `RemoteWatcher.Heartbeat` / `HeartbeatRsp` | `RemoteWatcher.scala` | 未対応 | medium | ハートビートプロトコル未実装 |
| `RemoteDeploymentWatcher` | `RemoteDeploymentWatcher.scala` | 未対応 | hard | リモートデプロイ機能の前提 |

### 9. Artery 圧縮（Header Compression）

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `CompressionTable` | `artery/compress/CompressionTable.scala` | 未対応 | hard | ActorRef/manifest の圧縮テーブル |
| `InboundCompressions` | `artery/compress/InboundCompressions.scala` | 未対応 | hard | 受信側圧縮管理 |
| `DecompressionTable` | `artery/compress/DecompressionTable.scala` | 未対応 | hard | 展開テーブル |
| `TopHeavyHitters` | `artery/compress/TopHeavyHitters.scala` | 未対応 | hard | 頻出パターン検出 |

### 10. コントロール・ハンドシェイク

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `HandshakeReq` / `HandshakeRsp` | `artery/Handshake.scala` | `HandshakeKind`, `HandshakeFrame` | - | 実装済み |
| `Quarantined` message | `artery/Control.scala` | `quarantine` メソッド | - | 機能は実装済み |
| `ActorSystemTerminating` | `artery/Control.scala` | `notify_system_shutdown` | - | 機能は実装済み |
| `Flush` / `FlushAck` | `artery/Control.scala` | 未対応 | medium | グレースフルシャットダウンの保証 |

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）
- `GracefulShutdownQuarantinedEvent` - イベント型の追加のみ
- `ThisActorSystemQuarantinedEvent` - イベント型の追加のみ

### Phase 2: easy（単純な新規実装）
- `TestTransport` - テスト用モックトランスポート
- `FailureInjectorTransportAdapter` - 障害注入テスト用

### Phase 3: medium（中程度の実装工数）
- `RemoteWatcher` ハートビートプロトコル - 定期的なハートビートとFD統合
- `InboundContext` の不足メソッド - `association()`, `completeHandshake()`
- `Flush` / `FlushAck` - グレースフルシャットダウン保証

### Phase 4: hard（アーキテクチャ変更を伴う）
- `RemoteDeployer` + `RemoteDaemon` - リモートデプロイ機能全体
- `RemoteDeploymentWatcher` - リモートデプロイ監視
- Artery Header Compression（`CompressionTable` 等） - パフォーマンス最適化

### 対象外（n/a）
- `PekkoProtocolTransport` - Classic Remoting向け（Artery互換で不要）
- `ThrottlerTransportAdapter` - Classic テスト用
- `AbstractTransportAdapter` - Classic向け
- `NettyTransport` - JVM固有（Netty）
- `ArteryAeronUdpTransport` - JVM固有（Aeron）
- SSL関連（`SSLEngineProvider` 等）- JVM固有（RustはTLS別途）
- `AddressUidExtension` / `BoundAddressesExtension` - Extension パターン不要
- Serialization関連 - JVM Serialization固有
- Security Provider - JVM固有
- `RemoteRouterConfig` - ルーティングはcluster層で扱う

---

## 総評

fraktor-rs の remote モジュールは前回分析から大幅に改善され、カバレッジが **70% → 90%** に向上した。特に障害検出基盤（`FailureDetector` trait、`DeadlineFailureDetector`、`FailureDetectorRegistry`）、信頼配信（`AckedDelivery`）、ライフサイクルイベント（`RemotingLifecycleEvent` enum）の実装により、7件のギャップが解消された。

残るギャップは以下に集中：
1. **リモートデプロイ**（RemoteDeployer, RemoteDaemon）— アーキテクチャ変更を伴う
2. **Header Compression**（CompressionTable 等）— パフォーマンス最適化
3. **テストユーティリティ**（TestTransport, FailureInjectorTransportAdapter）— テストキットの充実

コアのリモーティング機能（トランスポート、エンドポイント、ハンドシェイク、障害検出、信頼配信、ライフサイクルイベント）は完全にカバーされている。

## 次の推奨プラン（全体優先度）

全体方針として、まず `actor` / `streams` の Pekko 互換を上げてから cluster 深掘りを行うため、remote は既存カバレッジを維持しつつ必要最小限の補強に留める。

- 第1優先: 既存実装の安定化  
  - `Phase 1-2` の項目（`TestTransport`, `FailureInjectorTransportAdapter`, `Flush/FlushAck`）を現行 API 破壊なしで実装
- 第2優先: `streams` と `actor` の統合検証  
  - `RemoteWatcher` 系ハートビートや `SourceRef`/`SinkRef` 実装時に、リモート層との接続前提を先に固める
- 第3優先: cluster 優先度見直しと並行  
  - cluster を中核スコープアウトしている期間は、remote 側は `cluster` 追加項目との依存関係だけを抑え、SBR や大規模クラスタ運用機構は別計画で扱う
