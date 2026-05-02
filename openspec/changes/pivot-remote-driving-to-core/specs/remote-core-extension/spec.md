## ADDED Requirements

### Requirement: RemoteEvent enum の存在

`fraktor_remote_core_rs::core::extension::RemoteEvent` enum が定義され、adapter から core への通知種別を closed enum として表現する SHALL。

#### Scenario: RemoteEvent の存在

- **WHEN** `modules/remote-core/src/core/extension/remote_event.rs` を読む
- **THEN** `pub enum RemoteEvent` が定義されている

#### Scenario: 必要なバリアントの宣言

- **WHEN** `RemoteEvent` のバリアント一覧を検査する
- **THEN** 少なくとも以下を含む
  - `InboundFrameReceived { authority: TransportEndpoint, frame: alloc::vec::Vec<u8> }`
  - `OutboundFrameAcked { authority: TransportEndpoint, sequence: u64 }`
  - `HandshakeTimerFired { authority: TransportEndpoint, generation: u64 }`
  - `QuarantineTimerFired { authority: TransportEndpoint }`
  - `ConnectionLost { authority: TransportEndpoint, cause: ConnectionLostCause }`
  - `TransportShutdown`
  - `BackpressureCleared { authority: TransportEndpoint }`

#### Scenario: open hierarchy の不在

- **WHEN** `RemoteEvent` の定義を検査する
- **THEN** `#[non_exhaustive]` および unbounded な generic は宣言されていない（closed enum として固定する）

#### Scenario: generation 型は u64

- **WHEN** `HandshakeTimerFired` バリアントの `generation` フィールド型を検査する
- **THEN** 型は `u64` であり、`HandshakeGeneration` 等の newtype でラップされていない

### Requirement: RemoteEventSource trait

`fraktor_remote_core_rs::core::extension::RemoteEventSource` trait が定義され、`Remote::run` が消費する Port を表現する SHALL。

#### Scenario: trait の存在

- **WHEN** `modules/remote-core/src/core/extension/remote_event_source.rs` を読む
- **THEN** `pub trait RemoteEventSource: Send` が定義されている

#### Scenario: recv のシグネチャ

- **WHEN** `RemoteEventSource::recv` の定義を読む
- **THEN** `fn recv(&mut self) -> impl core::future::Future<Output = Option<RemoteEvent>> + Send + '_` または `async fn recv(&mut self) -> Option<RemoteEvent>` 形式で宣言されている

#### Scenario: tokio 非依存

- **WHEN** `modules/remote-core/src/core/extension/` 配下の RemoteEvent / RemoteEventSource 関連 import を検査する
- **THEN** `tokio` クレートへの参照が存在しない

#### Scenario: RemoteEventSink trait の不在

- **WHEN** `modules/remote-core/src/core/extension/` 配下のソースを検査する
- **THEN** `pub trait RemoteEventSink` または同等の adapter→core push 用 trait が定義されていない（adapter 内部 sender で完結し、純増ゼロ方針を維持する）

### Requirement: Remote::run は inherent async method として駆動主導権を持つ

`Remote<I>` 構造体に inherent method `pub async fn run<S: RemoteEventSource>(&mut self, source: &mut S) -> Result<(), RemotingError>` が定義され、event loop の主導権を core 側に集約する SHALL。`Remoting` trait に async fn を追加してはならない（MUST NOT）。

#### Scenario: Remote::run のシグネチャ

- **WHEN** `modules/remote-core/src/core/extension/remote.rs` を読む
- **THEN** `impl<I: RemoteInstrument> Remote<I>` ブロックに `pub async fn run<S>(&mut self, source: &mut S) -> Result<(), RemotingError>` または同等のシグネチャが宣言されている
- **AND** `S: RemoteEventSource` が trait bound として要求される

#### Scenario: Remoting trait に async fn を追加しない

- **WHEN** `Remoting` trait のメソッド一覧を検査する
- **THEN** `async fn` は存在せず、戻り値に `Future` を含まない（既存の `start` / `shutdown` / `quarantine` / `addresses` のみ）

#### Scenario: source 枯渇で Ok(())

- **WHEN** `RemoteEventSource::recv` が `None` を返す
- **THEN** `Remote::run` は `Ok(())` を返してループ終了する

#### Scenario: TransportShutdown で Ok(())

- **WHEN** source から `RemoteEvent::TransportShutdown` を受信する
- **THEN** `Remote::run` は `Ok(())` を返してループ終了する

#### Scenario: 復帰不能エラーで Err

- **WHEN** event 処理中に transport が永続的に失敗するなど復帰不能なエラーが発生する
- **THEN** `Remote::run` は `Err(RemotingError::TransportUnavailable)` または同等の variant を返してループ終了する
- **AND** 戻り値の `Result` を `let _ = ...` で握りつぶす経路は呼び出し側に存在しない

### Requirement: 別 Driver 型を新設しない

`Remote::run` の責務を担う `RemoteDriver` / `RemoteDriverHandle` / `RemoteDriverOutcome` 等の新規型を core 側に追加してはならない（MUST NOT）。これらの責務は `Remote<I>` の inherent method と既存 `Remoting` trait と `Result<(), RemotingError>` で表現する。

#### Scenario: RemoteDriver 型の不在

- **WHEN** `modules/remote-core/src/core/` 配下を検査する
- **THEN** `pub struct RemoteDriver` または `pub mod driver` が定義されていない

#### Scenario: RemoteDriverHandle 型の不在

- **WHEN** `modules/remote-core/src/core/` 配下を検査する
- **THEN** `pub struct RemoteDriverHandle` が定義されていない

#### Scenario: RemoteDriverOutcome enum の不在

- **WHEN** `modules/remote-core/src/core/` 配下を検査する
- **THEN** `pub enum RemoteDriverOutcome` が定義されていない（`Result<(), RemotingError>` で「正常終了 / 異常終了」を表現する）

### Requirement: AssociationEffect::StartHandshake は Remote::run で実行される

`Remote::run` のループ内で `AssociationEffect::StartHandshake { authority, timeout, generation }` を `RemoteTransport` 経由の handshake 開始に dispatch する SHALL。adapter 側の effect application からは該当分岐を削除する。

#### Scenario: Remote::run による StartHandshake 実行

- **WHEN** `Remote::run` が effect 列処理で `AssociationEffect::StartHandshake { authority, timeout, generation }` を見つける
- **THEN** `RemoteTransport` 経由で handshake request を送出する
- **AND** generation 値はそのまま adapter 側に伝わり、handshake timer task の管理に使われる

### Requirement: Codec 経路の明文化

`Remote::run` は inbound 側で raw frame を `Codec::decode` で復号してから `Association` に渡し、outbound 側で `Association::next_outbound` の戻り値を `Codec::encode` で raw bytes 化してから `RemoteTransport` に渡す SHALL。

#### Scenario: inbound decode の経路

- **WHEN** `Remote::run` が `RemoteEvent::InboundFrameReceived { authority, frame }` を受信する
- **THEN** `Codec<InboundEnvelope>::decode(&frame)` で復号する
- **AND** 復号した `InboundEnvelope` を該当 association の dispatch 経路に渡す

#### Scenario: outbound encode の経路

- **WHEN** `Remote::run` が `Association::next_outbound()` で `OutboundEnvelope` を取得する
- **THEN** `Codec<OutboundEnvelope>::encode(&envelope)` で raw bytes 化する
- **AND** その raw bytes を `RemoteTransport::send` または同等の API に渡す

### Requirement: outbound watermark backpressure の発火経路

`Remote::run` は `Association::total_outbound_len()` を `RemoteConfig::outbound_high_watermark` / `outbound_low_watermark` と比較し、enqueue / dequeue のたびに `Association::apply_backpressure(BackpressureSignal::Apply)` または `Release` を発火する SHALL。

#### Scenario: high watermark で Apply

- **WHEN** `Remote::run` が outbound enqueue 後に `Association::total_outbound_len()` を確認し、`outbound_high_watermark` を超える
- **THEN** `Remote::run` は `Association::apply_backpressure(BackpressureSignal::Apply)` を呼ぶ
- **AND** 該当 instrument の `record_backpressure(.., BackpressureSignal::Apply, ..)` が呼ばれる

#### Scenario: low watermark で Release

- **WHEN** `Remote::run` が outbound dequeue 後に `Association::total_outbound_len()` を確認し、`outbound_low_watermark` を下回り、かつ直前の状態が Apply 中だった
- **THEN** `Remote::run` は `Association::apply_backpressure(BackpressureSignal::Release)` を呼ぶ
- **AND** 該当 instrument の `record_backpressure(.., BackpressureSignal::Release, ..)` が呼ばれる

#### Scenario: 設定値の経路

- **WHEN** `RemoteConfig` のフィールドを検査する
- **THEN** `pub outbound_high_watermark: usize` と `pub outbound_low_watermark: usize` が宣言され、`outbound_low_watermark < outbound_high_watermark` を validation する

### Requirement: 戻り値の握りつぶし禁止

`Remote::run` 内で `RemoteEventSource::recv`（戻り値 `Option`）以外の `Result` 戻り値（`RemoteTransport::*`、`Codec::*` 等）を `let _ = ...` で握りつぶしてはならない（MUST NOT）。

#### Scenario: 戻り値の明示的扱い

- **WHEN** `Remote::run` の実装ソースを検査する
- **THEN** `let _ = ...` による `Result` 握りつぶしが存在しない
- **AND** 失敗は `?` で伝播するか、`match` で観測可能な経路（log / metric / instrument）に分岐する
