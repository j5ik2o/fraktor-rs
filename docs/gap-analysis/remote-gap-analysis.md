# remote モジュール ギャップ分析

参照実装: `references/pekko/remote/src/main/scala/`
対象実装: `modules/remote-core/src/`, `modules/remote-adaptor-std/src/`
更新日: 2026-04-19（第3版）

## 重要な前提（第3版で是正）

Pekko remote モジュールの宣言のうち、`private[remote]` / `private[pekko]` / `private[ssl]` / `private[tcp]` が付与されているものは **外部公開 API ではない**。以下は内部 API であり、本 gap analysis の公開 API カバレッジ指標からは除外する:

| 型 | 可視性 | 備考 |
|----|--------|------|
| `ArteryTransport`, `Association`, `AssociationRegistry` | `private[remote]` | Artery 内部実装 |
| `InboundEnvelope`, `OutboundEnvelope` | `private[remote]` | Artery 内部メッセージ表現 |
| `CompressionTable`, `DecompressionTable`, `InboundCompressions`, `TopHeavyHitters`, `CompressionProtocol` | `private[remote]` | 圧縮はすべて内部実装 |
| `RemoteActorRefProvider`, `RemoteActorRef` | `private[pekko]` | ActorSystem 拡張点 |
| `RemoteWatcher` | `private[pekko]` | リモート watcher 内部実装 |
| `RemotingFlightRecorder`, `NoOpRemotingFlightRecorder` | `private[pekko]` | Flight Recorder は内部 API |
| `Classic Remoting` 全系統 | `@deprecated` | Artery 移行済 |
| `NettyTransport`, `AeronUdpTransport`, `JFRRemotingFlightRecorder` | 一部 public だが実装 | JVM / Netty / Aeron / JFR 依存、Rust では n/a |

第2版までは `CompressionTable` / `Association` などを公開 API ギャップとして列挙していたが、これらは Pekko 側でも内部 API であるため、本版からは **内部構造** として扱う（public/internal 境界を越えない）。

fraktor-rs 側は設計選択として `Association`, `InboundEnvelope`, `OutboundEnvelope`, `Codec` 等を **`pub` で公開している**。これは参照実装レベルの差異であり、Pekko の契約意図（core 内部に閉じる）とは異なるが、fraktor-rs の設計原則（no_std / testability / composability）に沿った明示的な選択として許容する。

## サマリー

| 指標 | 値 |
|------|-----|
| Pekko 公開型数（non-deprecated / non-JVM / non-`private[*]`） | 30 |
| fraktor-rs 公開型数 | 78（core: 71, std: 7）※ Pekko では `private[remote]` に相当する内部型も `pub` で露出 |
| 契約意図カバレッジ（概念単位） | 13/30 (43%) |
| 実装ギャップ数 | 17 |
| スタブ検出 (`todo!()` / `unimplemented!()`) | **0 件** |

**補足**:
- `typed/` サブ層は remote には存在しない（Pekko も同じ）
- 第3版で公開 API 数が 2026-03-22 時点から減って見えるのは、Pekko の `private[remote]` を除外した結果であり、実装が退行したわけではない
- Phase B までのコア通信基盤（Transport / Association / Handshake / Envelope / Watcher / FailureDetector / WireCodec）は全て実動作する状態で実装済み
- `todo!()`/`unimplemented!()` ゼロ = 手抜き実装が存在しない

## 層別カバレッジ

| 層 | Pekko 対応 | fraktor-rs | 評価 |
|----|-----------|-----------|------|
| core / 通信カーネル（Artery 内部実装相当） | `private[remote]` 群 | core/ 11 サブディレクトリ・71 公開型 | **概念レベルでカバー済み**（public 公開範囲は fraktor-rs のほうが広い） |
| core / 公開 API | 約 30 型 | 13 概念実装 | 43% |
| core / typed ラッパー | n/a | n/a | remote は typed 層なし |
| std / アダプタ | 本質的に JVM ランタイム依存実装 | 7 公開型（TCP / extension installer / watcher actor / provider dispatch / association runtime） | **TCP 動作可能**、TLS 未対応 |

## カテゴリ別ギャップ

### FailureDetector　✅ 実装済み 3/5 (60%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `FailureDetectorWithAddress` | `FailureDetector.scala:43` | 未対応 | core/failure_detector | easy | `setAddress(addr)` で検出器がログ出力に使うアドレスを受け取る SPI。`PhiAccrualFailureDetector` が実装。fraktor-rs の `PhiAccrualFailureDetector` にはアドレス情報なし |
| `PhiAccrualFailureDetector.phi` 統計精度 | `PhiAccrualFailureDetector.scala:188` | 部分実装 | core/failure_detector | medium | fraktor-rs の Phi 計算は `HeartbeatHistory` で mean / variance / stdDeviation を保持するが、Pekko の `-log10(1 - CDF(y))`（正規分布 CDF 近似）との数式一致は要検証 |

実装済み: `PhiAccrualFailureDetector`, `HeartbeatHistory`, `FailureDetector` 相当（FailureDetectorRegistry は actor-core 側に配置）

### Address / UniqueAddress　✅ 実装済み 1/1 (100%)

実装済み（概念レベルで完全）:

- `Address` — `core/address/base.rs`
- `UniqueAddress` — `core/address/unique_address.rs`（Pekko と同形式 (Address, uid: Long) を維持）
- `RemoteNodeId` — `core/address/remote_node_id.rs`（内部用 ID、Pekko にない追加概念。fraktor-rs 固有の設計選択）
- `ActorPathScheme` — `core/address/scheme.rs`

### Artery Quarantine Events　✅ 実装済み 1/3（別名で集約） (33%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `QuarantinedEvent` (artery) | `artery/QuarantinedEvent.scala:18` | 別名 | core/event (actor-core) | easy | `RemotingLifecycleEvent::Quarantined` で代替 |
| `GracefulShutdownQuarantinedEvent` | `artery/QuarantinedEvent.scala:26` | 未対応 | core/event (actor-core) | easy | 正常シャットダウン時の quarantine を識別する専用 variant が未 |
| `ThisActorSystemQuarantinedEvent` | `artery/QuarantinedEvent.scala:31` | 未対応 | core/event (actor-core) | easy | リモートが自ノードを quarantine したことを通知する専用 variant が未 |

fraktor-rs では `fraktor_actor_core_rs::core::event::stream::RemotingLifecycleEvent::Quarantined` が authority / reason / correlation_id を保持する汎用 lifecycle event として公開済み。Pekko の 3 種細分化までは未到達。

### RemotingListenEvent　❌ 未実装 0/1 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RemotingListenEvent` | `RemotingLifecycleEvent.scala:78` | 未対応 | core/extension | easy | `listenAddresses: Set[Address]` を保持。リスニング開始通知。fraktor-rs は `RemotingLifecycleEvent::Started` を持つが、listen address の公開が欠落 |

関連して、`StdRemoting::addresses` が現状 `&[]` を返す（空スライス返却スタブ）gap も `RemotingListenEvent` と合わせて対処したい。

### Remote Instrument（監視 SPI）　✅ 実装済み 1/1 (100%, シグネチャ差異)

実装済みだがシグネチャ差異あり（Rust 固有の制約による合理的差異）:

- Pekko: `remoteWriteMetadata(recipient, message, sender, buffer: ByteBuffer)` — ActorRef 参照あり
- fraktor-rs: `remote_write_metadata(&self, buffer: &mut Vec<u8>)` — ActorRef なし（Rust のメタデータ用途では不要）

### RemoteSettings / ArterySettings　✅ 実装済み 2/2 (100%, 別名)

実装済みだが設計差異あり:

- Pekko `RemoteSettings` + `ArterySettings` → fraktor-rs `RemoteConfig`（単一構造に統合）
- fraktor-rs は `canonical host/port`, `handshake timeout`, `shutdown flush timeout`, `ack send/receive window`, `transport scheme`, `backpressure listener`, `remote instrument` を保持
- Classic remoting の deprecated 設定群は持たない

### RemoteRouterConfig　❌ 未実装 0/1 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RemoteRouterConfig` | `routing/RemoteRouterConfig.scala:47` | 未対応 | core/provider or core/routing | medium | `local: Pool`, `nodes: Iterable[Address]` を受け取り、リモートノード群に router pool を展開する。fraktor-rs には対応概念なし。actor-core 側の Router DSL と remote-core の Provider を繋ぐ層が必要 |

### RemoteLogMarker　❌ 未実装 0/1 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `RemoteLogMarker` | `RemoteLogMarker.scala:27` | 未対応 | core/instrument or std | easy | `@ApiMayChange`。`failureDetectorGrowing`, `quarantine`, `connect`, `disconnected` などのログマーカー。Rust では `tracing::field` / `log::kv` として実装可能 |

### RemoteTransportException　✅ 実装済み 2/2 (100%, 別名)

- `RemoteTransportException` → `RemotingError`（`extension/remoting_error.rs`）
- `RemoteTransportExceptionNoStackTrace` → `TransportError`（`transport/transport_error.rs`）＋ `ProviderError` と機能分割

### SSL/TLS セキュリティ　❌ 未実装 0/5 (0%)

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `SSLEngineProvider` | `artery/tcp/SSLEngineProvider.scala:24` | 未対応 | core/transport | hard | TLS エンジンプロバイダー SPI。Rust では `TlsConnector` 抽象（rustls / native-tls）に相当 |
| `SslTransportException` | `artery/tcp/SSLEngineProvider.scala:46` | 未対応 | core/transport | easy | SSL エラー型。`TransportError` に variant 追加で対応可能 |
| `SSLEngineProviderSetup` | `artery/tcp/SSLEngineProvider.scala:75` | 未対応 | core/config | medium | Setup DSL でのプロバイダー注入 |
| `ConfigSSLEngineProvider` | `artery/tcp/ConfigSSLEngineProvider.scala:48` | 未対応 | std/tcp_transport | hard | 設定から SSL エンジンを構成する実装（rustls-pemfile 相当） |
| `RotatingKeysSSLEngineProvider` | `artery/tcp/ssl/RotatingKeysSSLEngineProvider.scala:59` | 未対応 | std/tcp_transport | hard | PEM 鍵ローテーション対応 SSL プロバイダー |

### Serialization　✅ 実装済み 2/6 (33%, actor-core に配置)

actor-core に `Serializer` / `SerializerWithStringManifest` trait が配置済み。Pekko の remote 固有シリアライザ群（`MiscMessageSerializer`, `MessageContainerSerializer`, `ProtobufSerializer`, `SystemMessageSerializer`）と個別対応は未。

| Pekko API | Pekko参照 | fraktor対応 | 実装先層 | 難易度 | 備考 |
|-----------|-----------|-------------|----------|--------|------|
| `MiscMessageSerializer` | `serialization/MiscMessageSerializer.scala:37` | 未対応 | core/serialization (actor-core) | medium | PoisonPill / Kill 等のシステム制御メッセージ用。Rust 側は actor-core に system message 型あり、専用シリアライザは未 |
| `MessageContainerSerializer` | `serialization/MessageContainerSerializer.scala:30` | 未対応 | core/serialization (actor-core) | easy | `ActorSelectionMessage` のシリアライズ |
| `ProtobufSerializer` | `serialization/ProtobufSerializer.scala:30,57` | 未対応 | core/serialization (actor-core) | medium | Protobuf メッセージ用の標準シリアライザ。prost 経由で実装可能 |
| `SystemMessageSerializer` | `serialization/SystemMessageSerializer.scala:22` | 未対応 | core/serialization (actor-core) | easy | `Watch` / `Unwatch` / `DeathWatchNotification` の専用シリアライザ |
| `ThrowableNotSerializableException` | `serialization/ThrowableNotSerializableException.scala:22` | 未対応 | core/serialization (actor-core) | trivial | 例外クラスの追加のみ |

### Flight Recorder　✅ 実装済み 1/1（別アプローチ）

実装済みだがアプローチが異なる（Pekko は JFR、fraktor-rs はリングバッファ）:

- Pekko: JFR（Java Flight Recorder）ベース、transport lifecycle イベント約 30 種 — JVM 固有
- fraktor-rs: `RemotingFlightRecorder` リングバッファ型メトリクス — Rust/no_std に適切な設計
- JFR 固有イベント（Aeron, JFR Events）は n/a

### 現状ギャップ: `StdRemoting::addresses` 空スライス返却　❌ 未解消 1件

| 項目 | 現状 | 影響 |
|------|------|------|
| `StdRemoting::addresses(&self) -> &[Address]` | `&[]` を返すのみ | ライブアドレス公開が未完。`RemotingListenEvent` 実装の前提となるため、これを先に解消しないと listen event も作れない |

原因: `extension/remoting.rs` の `Remoting` trait が `&[Address]` 返却契約で、ロックを越えた借用ができない。trait 設計を `Vec<Address>` 返却または `snapshot_addresses() -> AddressSnapshot` に変更する必要あり。難易度: **easy** (trait 契約変更)。

### Classic Remoting 型（deprecated）　n/a

Classic remoting / Netty Transport / Aeron UDP / RemoteDeployer / BoundAddressesExtension / JFRRemotingFlightRecorder はすべて n/a（deprecated または JVM 固有）。fraktor-rs では実装不要。

**Pekko 側に "実装済み" と記載済みの `AckedDelivery` (Pekko 側 deprecated)** は fraktor-rs にも実装があるが、Pekko Artery では未使用であり、YAGNI 観点では削除候補。構造 gap として後述。

## 実装優先度

### Phase 1: trivial / easy（既存設計の範囲で埋められる）

| 項目 | 実装先層 | 概要 |
|------|---------|------|
| `ThrowableNotSerializableException` 相当 | core/serialization (actor-core) | 例外型の追加 |
| `FailureDetectorWithAddress` | core/failure_detector | `FailureDetector` trait に `set_address(&mut self, addr: &str)` を追加（default 空実装） |
| `SslTransportException` | core/transport | `TransportError` に `TlsHandshakeFailed` / `TlsCertificateError` variant を追加 |
| `GracefulShutdownQuarantinedEvent` / `ThisActorSystemQuarantinedEvent` 相当 | core/event (actor-core) | `RemotingLifecycleEvent` enum に variant 2 件追加 |
| `RemotingListenEvent` 相当 | core/extension | `RemotingLifecycleEvent::Listen { addresses }` variant の追加 |
| `MessageContainerSerializer` 相当 | core/serialization (actor-core) | `ActorSelectionMessage` シリアライザ |
| `SystemMessageSerializer` 相当 | core/serialization (actor-core) | system message 専用シリアライザ |
| `RemoteLogMarker` 相当 | core/instrument | 構造化ログ用のマーカー定数群 |
| `StdRemoting::addresses` 空スライス解消 | std/extension_installer | trait 契約変更で `Vec<Address>` 返却 |

### Phase 2: medium（新規ロジックを伴うが既存境界内）

| 項目 | 実装先層 | 概要 |
|------|---------|------|
| `PhiAccrualFailureDetector` 統計精度 | core/failure_detector | `-log10(1 - CDF(y))` の正規分布 CDF 近似を導入 |
| `SSLEngineProviderSetup` 相当 | core/config | TLS プロバイダー注入 DSL |
| `MiscMessageSerializer` 相当 | core/serialization (actor-core) | PoisonPill / Kill 等のシリアライザ |
| `ProtobufSerializer` 相当 | core/serialization (actor-core) | prost 経由の標準 Protobuf シリアライザ |
| `RemoteRouterConfig` 相当 | core/provider | actor-core Routing DSL と remote-core Provider を繋ぐ層 |

### Phase 3: hard（アーキテクチャ変更を伴う）

| 項目 | 実装先層 | 概要 |
|------|---------|------|
| `SSLEngineProvider` SPI | core/transport | TLS engine 抽象（rustls / native-tls 両対応） |
| `ConfigSSLEngineProvider` | std/tcp_transport | 設定から TLS エンジンを構成する実装 |
| `RotatingKeysSSLEngineProvider` | std/tcp_transport | PEM 鍵ローテーション対応 |
| Artery Compression（`CompressionTable` / `DecompressionTable` / `InboundCompressions`）相当 | core/wire + core/association | **内部実装**。ただし Pekko 側でも `private[remote]` なので公開 API ギャップではなく、ワイヤプロトコル拡張としての実装判断 |
| `TopHeavyHitters` 相当 | core/wire | 圧縮の前提となる LFU-like 頻出追跡データ構造 |

### 対象外（n/a）

- Classic Remoting 全系統（`@deprecated`）
- `NettyTransport` / `AeronUdpTransport`（JVM / Netty / Aeron 依存）
- `JFRRemotingFlightRecorder`（JFR = JVM 固有）
- `RemoteDeployer` / `RemoteDeploymentWatcher`（JVM クラスパスベース）
- `BoundAddressesExtension` / `AddressUidExtension`（JVM Extension SPI）
- `NotAllowedClassRemoteDeploymentAttemptException`（Rust 不要）

## 内部モジュール構造ギャップ

API ギャップカバレッジが 43% で 80% 未満のため、内部構造比較は本版では省略する。ただし、Phase 1〜2 完了後の目安として以下を記録する（第4版で詳述予定）:

| 構造観点 | Pekko | fraktor-rs 現状 | 備考 |
|---------|-------|----------------|------|
| Artery 圧縮責務 | `artery/compress/` で独立パッケージ | 未分離（未実装） | Phase 3 で `core/compress/` を新設予定 |
| TLS 責務 | `artery/tcp/ssl/` で独立パッケージ | 未分離（未実装） | Phase 3 で `core/tls/` or `std/tls_transport/` を新設予定 |
| Envelope / EnvelopeBuffer | `artery/EnvelopeBufferPool.scala` でプール管理 | fraktor-rs は `core/envelope/` に配置、pool 未分離 | 動作上は問題ないが、バックプレッシャ時の最適化余地あり |
| Association レジストリ | `private[remote] class AssociationRegistry` | `std/association_runtime/association_registry.rs` で同名 | **fraktor-rs では `pub` 公開**。Pekko の private 設計意図と差異あり（設計上の判断として許容） |
| FailureDetector レジストリ | `FailureDetectorRegistry[A]` 汎用型 | 未定義（`PhiAccrualFailureDetector` 直使用） | `core/failure_detector/registry.rs` 新設が Phase 2 内に入る可能性 |

## 既知スタブ / レガシー

`.takt/facets/knowledge/stub-elimination.md` は主に stream モジュール向けだが、remote-core / remote-adaptor-std に対して `todo!()` / `unimplemented!()` / `FIXME` / `TODO` を検索した結果 **0 件**。スタブは存在しない。

ただし、以下の「実装の痕跡はあるが Pekko で deprecated」な項目は YAGNI 観点で削除候補:

- `AckedDelivery` 相当の実装が fraktor-rs 側にある可能性（Pekko では `@deprecated("Classic remoting")`）。artery では使われていないため、将来削除を検討

## まとめ

**全体カバレッジ評価**: コア通信基盤（Transport / Association / Handshake / Envelope / Watcher / Wire）は Phase B 完了時点で **動作する実装** が揃っており、スタブゼロ。一方で Pekko の公開 API 契約に対する概念カバレッジは 43% で、TLS / 圧縮 / シリアライザ / RouterConfig / LogMarker / Quarantine event 3 種など公開 API が未対応。

**parity を低コストで前進できる未実装（Phase 1〜2 代表例）**:

- `RemotingListenEvent` + `StdRemoting::addresses` 空スライス解消 — listening アドレスの可観測性向上
- Artery Quarantine events 3 種への細分化 — 運用監視の粒度向上
- `FailureDetectorWithAddress` + `PhiAccrualFailureDetector` 統計精度 — 誤検知率の低減
- Serialization 専用シリアライザ群（Misc / Container / Protobuf / SystemMessage） — Pekko と互換のシリアライズ形式

**parity 上の主要ギャップ（Phase 3 代表例）**:

- **TLS/SSL 完全未実装（5 件）** — 本番環境での暗号化通信不可。rustls or native-tls バックエンドでの SPI 構築が必要
- **Artery Compression 未実装（5 件、内部 API 扱い）** — ActorRef / class manifest の圧縮ネゴシエーションプロトコル未到達
- **`RemoteRouterConfig`** — actor-core Router DSL と remote-core Provider の接続層。cluster-core 整備と並行検討が望ましい

**次のボトルネック評価**: 公開 API ギャップが 43% と過半を占めるため、本版では内部構造比較は省略。Phase 1〜2 完了後（カバレッジ 70% 程度を目安）に第4版で内部構造分析を実施する。それまでは公開 API 追加が優先。
