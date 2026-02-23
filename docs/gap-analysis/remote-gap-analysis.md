# remote モジュール ギャップ分析

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（公開API + 主要内部型） | 約40型 |
| fraktor-rs 公開型数 | 65型 |
| カバレッジ（機能カテゴリ単位） | 7/10 (70%) |
| 主要ギャップ数 | 12 |

> 注: Pekkoのremoteモジュールは多くの型が `private[pekko]` / `private[remote]` で内部APIだが、fraktor-rsでは同等概念を公開型として実装している。型数の直接比較は参考値。

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
| `FailureDetector` trait | `FailureDetector.scala` | 未対応 | easy | 抽象trait。`isAvailable`, `isMonitoring`, `heartbeat` |
| `FailureDetectorWithAddress` trait | `FailureDetector.scala` | 未対応 | trivial | `setAddress` のみ追加 |
| `PhiAccrualFailureDetector` | `PhiAccrualFailureDetector.scala` | `PhiFailureDetector` | - | 実装済み（別名） |
| `DeadlineFailureDetector` | `DeadlineFailureDetector.scala` | 未対応 | easy | deadline ベースの単純な検出器 |
| `FailureDetectorRegistry[A]` trait | `FailureDetectorRegistry.scala` | 未対応 | medium | リソース別のFD管理。`isAvailable`, `heartbeat`, `remove`, `reset` |
| `DefaultFailureDetectorRegistry` | `DefaultFailureDetectorRegistry.scala` | 未対応 | medium | Registry の標準実装 |

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
| `TcpFraming` | `artery/tcp/TcpFraming.scala` | 未対応（暗黙的に実装） | n/a | フレーミングはTokioTcpTransport内部 |

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
| `SystemMessageEnvelope` | `artery/SystemMessageDelivery.scala` | 未対応 | medium | システムメッセージの信頼配信 |
| `AckedDelivery` | `AckedDelivery.scala` | 未対応 | medium | 確認応答付き配信。システムメッセージに必要 |

### 6. イベント・ライフサイクル

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `RemotingLifecycleEvent` (sealed) | `RemotingLifecycleEvent.scala` | 部分的 | easy | EventPublisherのメソッドとして存在するが、型付きイベントenumがない |
| `QuarantinedEvent` | `artery/QuarantinedEvent.scala` | `publish_quarantined` | - | EventPublisher経由で実装済み |
| `GracefulShutdownQuarantinedEvent` | `artery/QuarantinedEvent.scala` | 未対応 | trivial | イベント型の追加のみ |
| `ThisActorSystemQuarantinedEvent` | `artery/QuarantinedEvent.scala` | 未対応 | trivial | イベント型の追加のみ |
| `AssociatedEvent` / `DisassociatedEvent` | `RemotingLifecycleEvent.scala` | `publish_connected` | - | EventPublisher経由で部分的 |

### 7. 設定・ユーティリティ

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `RemoteSettings` | `RemoteSettings.scala` | `RemotingExtensionConfig` | - | 実装済み（Builder パターンに変換） |
| `ArterySettings` | `artery/ArterySettings.scala` | `RemotingExtensionConfig` に統合 | - | 実装済み |
| `UniqueAddress` | `UniqueAddress.scala` | `RemoteNodeId` | - | 別名で実装済み |
| `AddressUidExtension` | `AddressUidExtension.scala` | 未対応 | n/a | Extension パターンはfraktor-rsでは不要 |
| `BoundAddressesExtension` | `BoundAddressesExtension.scala` | 未対応 | n/a | Extension パターンはfraktor-rsでは不要 |

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
| `ControlMessage` trait | `artery/Control.scala` | 未対応（暗黙的） | easy | 制御メッセージの型階層 |
| `Quarantined` message | `artery/Control.scala` | `quarantine` メソッド | - | 機能は実装済み |
| `ActorSystemTerminating` | `artery/Control.scala` | `notify_system_shutdown` | - | 機能は実装済み |
| `Flush` / `FlushAck` | `artery/Control.scala` | 未対応 | medium | グレースフルシャットダウンの保証 |
| `RemoteInstrument` | `artery/RemoteInstrument.scala` | 未対応 | easy | カスタム計装フック |

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）
- `GracefulShutdownQuarantinedEvent` - イベント型の追加のみ
- `ThisActorSystemQuarantinedEvent` - イベント型の追加のみ
- `FailureDetectorWithAddress` trait - `setAddress` 1メソッドの trait

### Phase 2: easy（単純な新規実装）
- `FailureDetector` trait - 3メソッドの抽象trait。`PhiFailureDetector`をこのtraitに適合させる
- `DeadlineFailureDetector` - deadline ベースの単純な検出器
- `RemotingLifecycleEvent` enum - 型付きイベントの体系化
- `ControlMessage` trait - 制御メッセージの型階層
- `RemoteInstrument` - カスタム計装フック
- `TestTransport` - テスト用モックトランスポート

### Phase 3: medium（中程度の実装工数）
- `FailureDetectorRegistry` trait + `DefaultFailureDetectorRegistry` - リソース別FD管理
- `SystemMessageDelivery` + `AckedDelivery` - システムメッセージの信頼配信
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
