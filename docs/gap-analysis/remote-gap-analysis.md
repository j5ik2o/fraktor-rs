# remote モジュール ギャップ分析

参照実装: `references/pekko/remote/`
対象実装: `modules/remote/src/`

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（非deprecated） | 20 |
| fraktor-rs 公開型数 | 62 |
| カバレッジ（型単位） | 14/20 (70%) |
| ギャップ数 | 9 |

## カテゴリ別ギャップ

### フェイラーディテクター　✅ 実装済み 5/6 (83%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `FailureDetectorWithAddress` | `FailureDetector.scala:43` | 未対応 | easy | `setAddress(addr)` で検出器がログ出力に使うアドレスを受け取る SPI。`PhiAccrualFailureDetector` が実装。fraktor-rs の `PhiFailureDetector` にはアドレス情報なし |
| `PhiAccrualFailureDetector.phi`（統計的実装） | `PhiAccrualFailureDetector.scala:188` | 部分実装 | medium | fraktor-rs は `elapsed / mean` の単純比率。Pekko は `-log10(1 - CDF(y))`（正規分布 CDF 近似）を使い標準偏差・分散も計算。アルゴリズムが別物 |

実装済み: `FailureDetector`, `FailureDetectorRegistry`, `DeadlineFailureDetector`（+Config）, `PhiFailureDetector`（+Config、簡略版）, `DefaultFailureDetectorRegistry`

### ID型・UniqueAddress　✅ 実装済み 1/1（別名）

実装済みだが設計差異あり:

- Pekko `UniqueAddress` = `(Address, uid: Long)` — Address はアクターシステムアドレス（protocol/host/port/system）
- fraktor-rs `RemoteNodeId` = `(system, host, port, uid)` — より原始的な表現で実質同等

### Quarantine イベント　✅ 実装済み 0/3 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `QuarantinedEvent` | `artery/QuarantinedEvent.scala:18` | 未対応 | easy | artery（modern）の公開イベント型。fraktor-rs は `QuarantineReason`（記述子）のみ |
| `GracefulShutdownQuarantinedEvent` | `artery/QuarantinedEvent.scala:26` | 未対応 | easy | 正常シャットダウン時の quarantine イベント |
| `ThisActorSystemQuarantinedEvent` | `artery/QuarantinedEvent.scala:32` | 未対応 | easy | リモートが自ノードを quarantine したことを通知するイベント |

### Remote Instrument（監視 SPI）　✅ 実装済み 1/1（別名）

実装済みだがシグネチャ差異あり:

- Pekko: `remoteWriteMetadata(recipient, message, sender, buffer: ByteBuffer)` — ActorRef 参照あり
- fraktor-rs: `remote_write_metadata(&self, buffer: &mut Vec<u8>)` — ActorRef なし（Rust にはアクターシステム参照不要）
- セマンティクス上は同等。Rust 固有の制約による合理的な差異

### Flight Recorder　✅ 実装済み 1/1（別アプローチ）

実装済みだがアプローチが異なる:

- Pekko: JFR（Java Flight Recorder）ベース、transport lifecycle イベント（aeronSink*, aeronSource*, tcpInbound*, tcpOutbound*, transportXxx*）約 30 種のメソッド — JVM 固有技術
- fraktor-rs: リングバッファ型メトリクス（`record_backpressure`, `record_suspect`, `record_reachable`）— 3 種、Rust/no_std に適切な設計
- JFR 固有イベント（Aeron, JFR Events）は n/a

### SSL/TLS セキュリティ　✅ 実装済み 0/5 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `SSLEngineProvider` | `artery/tcp/SSLEngineProvider.scala:24` | 未対応 | hard | TLS エンジンプロバイダー SPI。Tokio の `rustls`/`native-tls` に相当 |
| `SslTransportException` | `artery/tcp/SSLEngineProvider.scala:46` | 未対応 | easy | SSL エラー型。`TransportError` に enum variant として追加可能 |
| `SSLEngineProviderSetup` | `artery/tcp/SSLEngineProvider.scala:75` | 未対応 | medium | Setup DSL でのプロバイダー注入 |
| `ConfigSSLEngineProvider` | `artery/tcp/ConfigSSLEngineProvider.scala:48` | 未対応 | hard | 設定から SSL エンジンを構成する実装 |
| `RotatingKeysSSLEngineProvider` | `artery/tcp/ssl/RotatingKeysSSLEngineProvider.scala:59` | 未対応 | hard | 証明書ローテーション対応 SSL プロバイダー |

### 圧縮（Artery Compression）　✅ 実装済み 0/5 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `CompressionTable` | `artery/compress/CompressionTable.scala` | 未対応 | hard | ActorRef / class manifest の圧縮テーブル |
| `DecompressionTable` | `artery/compress/DecompressionTable.scala` | 未対応 | hard | 展開テーブル |
| `CompressionProtocol` | `artery/compress/CompressionProtocol.scala` | 未対応 | hard | 圧縮ネゴシエーションプロトコル |
| `InboundCompressions` | `artery/compress/InboundCompressions.scala` | 未対応 | hard | 受信側圧縮ステート管理 |
| `TopHeavyHitters` | `artery/compress/TopHeavyHitters.scala` | 未対応 | medium | 頻出エントリ追跡（LFU-like） |

### Classic Remoting 型（deprecated）　n/a

| Pekko API | Pekko参照 | fraktor対応 | 難易度 | 備考 |
|-----------|-----------|-------------|--------|------|
| `RemotingLifecycleEvent` + サブ型 | `RemotingLifecycleEvent.scala` | n/a | n/a | `@deprecated("Classic remoting", "Akka 2.6.0")` — Artery に移行済み |
| `AckedDelivery`（SeqNo 等） | `AckedDelivery.scala:21` | 実装済み | n/a | Pekko では deprecated だが fraktor-rs では実装済み — YAGNI 観点で要検討 |
| `RemoteDeployer` / `RemoteDeploymentWatcher` | `RemoteDeployer.scala` | n/a | n/a | JVM クラスパスベースリモートデプロイ — Rust に不要 |
| `BoundAddressesExtension` / `AddressUidExtension` | 対応ファイル | n/a | n/a | JVM Extension SPI — fraktor-rs は `RemotingExtension` で代替 |
| `NotAllowedClassRemoteDeploymentAttemptException` | `RemoteDaemon.scala:288` | n/a | n/a | デプロイ制約例外 — Rust 不要 |

## 実装優先度の提案

### Phase 1: trivial（既存組み合わせで即実装可能）

- `SslTransportException` — `TransportError` に `TlsHandshakeFailed`、`TlsCertificateError` variant を追加

### Phase 2: easy（単純な新規実装）

- `QuarantinedEvent`、`GracefulShutdownQuarantinedEvent`、`ThisActorSystemQuarantinedEvent` — `QuarantineReason` を使ったイベント型 3 種の追加。イベントバスへの統合
- `FailureDetectorWithAddress` — `FailureDetector` トレイトに `set_address(&mut self, addr: &str)` メソッドを追加（デフォルト空実装）

### Phase 3: medium（中程度の実装工数）

- `PhiAccrualFailureDetector` の統計的 phi 実装 — 現在の `elapsed / mean` から `-log10(1 - CDF(y))` の正規分布 CDF 近似に変更。`HeartbeatHistory` 相当の rolling statistics（mean、variance、stdDeviation）追加が必要
- SSL/TLS プロバイダー SPI — `TlsEngineProvider` trait + `RemotingExtensionConfig` への統合（`tokio-rustls` または `native-tls`）

### Phase 4: hard（アーキテクチャ変更を伴う）

- SSL/TLS フル実装（`ConfigTlsEngineProvider`、`RotatingKeysTlsEngineProvider`）
- 圧縮（`CompressionTable`、`DecompressionTable`、`InboundCompressions`）— ワイヤプロトコル変更を伴う
- `TopHeavyHitters` — 単体では中程度だが圧縮の前提となる基盤

### 対象外（n/a）

- Classic Remoting 型（deprecated）— Pekko 自身が artery へ移行済み
- JFR（Java Flight Recorder）イベント群 — JVM 固有
- `RemoteDeployer` / `RemoteDeploymentWatcher` — Rust に不要
- Aeron UDP Transport — 専用 C ライブラリ依存、no_std 非対応
- `BoundAddressesExtension` / `AddressUidExtension` — JVM Extension SPI

## まとめ

**全体カバレッジ評価**: フェイラーディテクター・トランスポート抽象化・エンドポイント管理などコアの通信基盤は十分にカバー済み。Pekko の非 deprecated 公開 API に対して 70% をカバーし、Rust 固有の代替実装で補っている。

**即座に価値を提供できる未実装機能（Phase 1〜2）**:

- 3 種の Quarantine イベント型の追加 — イベントバス統合で可観測性が向上
- `FailureDetectorWithAddress` — ログの質の向上（アドレス情報付きで障害地点が明確になる）

**実用上の主要ギャップ（Phase 3〜4）**:

- **Phi 統計精度** — 現実装は線形比率で Pekko の正規分布 CDF 近似とは意味論が異なる。ネットワーク変動に対する感度が不正確で、誤検知率が上がる可能性がある
- **TLS/SSL なし** — 本番環境での暗号化通信が不可。クラスター接続を外部 TLS ターミネーターに委ねる運用が必要

**YAGNI 観点での省略推奨**:

- `AckedDelivery` は Pekko で deprecated 済み。fraktor-rs に実装があるが artery では使われていない。将来削除を検討すべき
- 圧縮機能（Phase 4）は大量メッセージングが必要な時点まで不要
- Aeron UDP は IoT/組込み以外では必要性が低い
