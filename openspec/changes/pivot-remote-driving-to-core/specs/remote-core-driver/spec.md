## ADDED Requirements

### Requirement: RemoteDriver 型の存在

`fraktor_remote_core_rs::core::driver::RemoteDriver` 型が定義され、`AssociationRegistry`、`RemoteTransport`、`Timer`、`RemoteInstrument`、`Codec` を所有して remote 駆動の主導権を持つ SHALL。Pekko Artery の inbound / outbound driver loop に対応する core 側 driver である。

#### Scenario: RemoteDriver の存在

- **WHEN** `modules/remote-core/src/core/driver/remote_driver.rs` を読む
- **THEN** `pub struct RemoteDriver` または同等のジェネリクス署名 `RemoteDriver<S, K, T, I, C>` が定義されている
- **AND** 内部に `AssociationRegistry`、`RemoteTransport`、`Timer`、`RemoteInstrument`、`Codec` 相当を保持する

#### Scenario: 内部可変性の不在

- **WHEN** `RemoteDriver` のフィールド型を検査する
- **THEN** 駆動状態を `Cell<T>`、`RefCell<T>`、`SpinSyncMutex<T>`、`AShared<T>` でラップしていない
- **AND** Driver 自身は `&mut self` ベースで動作する

### Requirement: RemoteDriver::run の駆動契約

`RemoteDriver::run` は `RemoteEventSource` を引数に取る async fn で、所有権を消費して `RemoteDriverOutcome` を返す SHALL。run の中で source からイベントを受信し、対応する `Association` メソッドへ dispatch して effect 列を実行する。

#### Scenario: run のシグネチャ

- **WHEN** `RemoteDriver::run` の定義を読む
- **THEN** `pub async fn run(self, source: S) -> RemoteDriverOutcome` または同等のシグネチャ（`self` を消費し、`RemoteDriverOutcome` を返す）が宣言されている
- **AND** 戻り値の `RemoteDriverOutcome` は `Shutdown { reason }` / `SourceExhausted` / `Aborted { error }` を含む closed enum である

#### Scenario: source 枯渇で SourceExhausted

- **WHEN** `RemoteEventSource::recv` が `None` を返す
- **THEN** `run` は `RemoteDriverOutcome::SourceExhausted` を返して終了する

#### Scenario: TransportShutdown で Shutdown

- **WHEN** source から `RemoteEvent::TransportShutdown` を受信する
- **THEN** `run` は `RemoteDriverOutcome::Shutdown { reason }` を返して終了する

### Requirement: RemoteEvent enum

`fraktor_remote_core_rs::core::driver::RemoteEvent` enum が定義され、adapter から core への通知種別を closed enum として表現する SHALL。

#### Scenario: RemoteEvent の存在

- **WHEN** `modules/remote-core/src/core/driver/remote_event.rs` を読む
- **THEN** `pub enum RemoteEvent` が定義されている

#### Scenario: 必要なバリアントの宣言

- **WHEN** `RemoteEvent` のバリアント一覧を検査する
- **THEN** 少なくとも以下を含む
  - `InboundFrameReceived { authority, frame }`
  - `OutboundFrameAcked { authority, sequence }`
  - `HandshakeTimerFired { authority, generation }`
  - `QuarantineTimerFired { authority }`
  - `ConnectionLost { authority, cause }`
  - `TransportShutdown`
  - `BackpressureCleared { authority }`

#### Scenario: open hierarchy の不在

- **WHEN** `RemoteEvent` の定義を検査する
- **THEN** `#[non_exhaustive]` および unbounded な generic は宣言されていない（closed enum として固定する）

### Requirement: RemoteEventSource trait

`fraktor_remote_core_rs::core::driver::RemoteEventSource` trait が定義され、Driver が消費する Port を表現する SHALL。

#### Scenario: trait の存在

- **WHEN** `modules/remote-core/src/core/driver/remote_event_source.rs` を読む
- **THEN** `pub trait RemoteEventSource` が定義されている

#### Scenario: recv のシグネチャ

- **WHEN** `RemoteEventSource::recv` の定義を読む
- **THEN** `fn recv(&mut self) -> impl Future<Output = Option<RemoteEvent>> + Send` または `async fn recv(&mut self) -> Option<RemoteEvent>` 形式で宣言されている

#### Scenario: tokio 非依存

- **WHEN** `modules/remote-core/src/core/driver/` 配下のすべての import を検査する
- **THEN** `tokio` クレートへの参照が存在しない

### Requirement: RemoteEventSink trait

`fraktor_remote_core_rs::core::driver::RemoteEventSink` trait が定義され、adapter が core 側へイベントを push する Port を表現する SHALL。

#### Scenario: trait の存在

- **WHEN** `modules/remote-core/src/core/driver/remote_event_sink.rs` を読む
- **THEN** `pub trait RemoteEventSink: Send + Sync` が定義されている

#### Scenario: push のシグネチャ

- **WHEN** `RemoteEventSink::push` の定義を読む
- **THEN** `fn push(&self, event: RemoteEvent) -> Result<(), RemoteEventDispatchError>` が宣言されている

#### Scenario: 戻り値を握りつぶさない契約

- **WHEN** `push` が `Result` を返す
- **THEN** 呼び出し側は `?` または `match` で扱い、`let _ = ...` での無言握りつぶしは禁止される（`.agents/rules/ignored-return-values.md` 準拠）

### Requirement: Timer Port

`fraktor_remote_core_rs::core::driver::Timer` trait が定義され、Driver が delayed event を予約する Port を表現する SHALL。

#### Scenario: trait の存在

- **WHEN** `modules/remote-core/src/core/driver/timer.rs` を読む
- **THEN** `pub trait Timer: Send + Sync` が定義されている

#### Scenario: schedule / cancel のシグネチャ

- **WHEN** `Timer` のメソッドを読む
- **THEN** `fn schedule(&self, delay: Duration, event: RemoteEvent) -> TimerToken` と `fn cancel(&self, token: TimerToken)` が宣言されている

#### Scenario: cancel の冪等性

- **WHEN** 同じ `TimerToken` に対して `cancel` を複数回呼ぶ
- **THEN** 2 回目以降は no-op として安全に動作する

#### Scenario: tokio 非依存

- **WHEN** `Timer` trait 定義ファイルの import を検査する
- **THEN** `tokio` クレートへの参照が存在しない

### Requirement: AssociationEffect::StartHandshake は Driver で実行される

`AssociationEffect::StartHandshake { endpoint }` は `RemoteDriver` 内で `RemoteTransport::initiate_handshake(endpoint)` に dispatch されなければならない（MUST）。adapter 側でこの effect を ignore する分岐を持ってはならない（MUST NOT）。

#### Scenario: Driver が StartHandshake を実行する

- **WHEN** `Association` の状態遷移メソッドが `AssociationEffect::StartHandshake { endpoint }` を返す
- **THEN** Driver は `RemoteTransport::initiate_handshake(&endpoint)` を呼ぶ
- **AND** 同時に `Timer::schedule(handshake_timeout, RemoteEvent::HandshakeTimerFired { .. })` で timeout を予約する

#### Scenario: adapter 側の StartHandshake ignore 分岐の不在

- **WHEN** `modules/remote-adaptor-std/src/std/effect_application.rs` の `apply_effects_in_place` を読む
- **THEN** `AssociationEffect::StartHandshake { .. } => /* ignore */` のような無視分岐が存在しない

### Requirement: Codec 経路の明文化

Driver は inbound 側で raw frame を `Codec::decode` で復号してから `Association` に渡し、outbound 側で `Association::next_outbound` の戻り値を `Codec::encode` で raw bytes 化してから `RemoteTransport` に渡す SHALL。

#### Scenario: inbound decode の経路

- **WHEN** Driver が `RemoteEvent::InboundFrameReceived { authority, frame }` を受信する
- **THEN** `Codec<InboundEnvelope>::decode(&frame)` で復号する
- **AND** 復号した `InboundEnvelope` を該当 association の dispatch 経路に渡す

#### Scenario: outbound encode の経路

- **WHEN** Driver が `Association::next_outbound()` で `OutboundEnvelope` を取得する
- **THEN** `Codec<OutboundEnvelope>::encode(&envelope)` で raw bytes 化する
- **AND** その raw bytes を `RemoteTransport::send_frame` または同等の API に渡す

### Requirement: outbound watermark backpressure

`RemoteConfig::outbound_high_watermark` と `outbound_low_watermark` を導入し、Driver は queue 長が high を超えたら `Association::apply_backpressure(Engaged)` を発火、low を下回ったら `apply_backpressure(Released)` を発火する SHALL。

#### Scenario: high watermark で Engaged

- **WHEN** Driver が outbound enqueue 後に `Association` の総 queue 長 (`control.len() + ordinary.len()`) を確認し、`outbound_high_watermark` を超える
- **THEN** Driver は `Association::apply_backpressure(BackpressureSignal::Engaged)` を呼ぶ
- **AND** 該当 instrument の `record_backpressure(.., BackpressureSignal::Engaged, ..)` が呼ばれる

#### Scenario: low watermark で Released

- **WHEN** Driver が outbound dequeue 後に `Association` の総 queue 長を確認し、`outbound_low_watermark` を下回り、かつ直前の状態が Engaged だった
- **THEN** Driver は `Association::apply_backpressure(BackpressureSignal::Released)` を呼ぶ
- **AND** 該当 instrument の `record_backpressure(.., BackpressureSignal::Released, ..)` が呼ばれる

#### Scenario: 設定値の経路

- **WHEN** `RemoteConfig` のフィールドを検査する
- **THEN** `pub outbound_high_watermark: usize` と `pub outbound_low_watermark: usize` が宣言され、`outbound_low_watermark < outbound_high_watermark` を validation する

### Requirement: RemoteDriverHandle と RemoteDriverOutcome

`fraktor_remote_core_rs::core::driver::RemoteDriverHandle` 型が定義され、`shutdown(reason)` で driver を停止し、`outcome().await` で `RemoteDriverOutcome` を取得する SHALL。

#### Scenario: RemoteDriverHandle の存在

- **WHEN** `modules/remote-core/src/core/driver/remote_driver_handle.rs` を読む
- **THEN** `pub struct RemoteDriverHandle` が定義され、`shutdown` と `outcome` メソッドを持つ

#### Scenario: shutdown が sink 経由で TransportShutdown を push する

- **WHEN** `RemoteDriverHandle::shutdown(reason)` を呼ぶ
- **THEN** 内部 sink へ `RemoteEvent::TransportShutdown` を push する
- **AND** 戻り値は `Result<(), RemoteEventDispatchError>` である

#### Scenario: outcome 取得

- **WHEN** Driver run 完了後に `RemoteDriverHandle::outcome().await` を呼ぶ
- **THEN** `RemoteDriverOutcome::Shutdown { reason }` / `SourceExhausted` / `Aborted { error }` のいずれかが返る

### Requirement: AssociationRegistry の所有権は Driver に集約される

`RemoteDriver` は `AssociationRegistry` を所有し、外部から共有されない SHALL。adapter 側はイベント push と I/O のみを担当し、`AssociationRegistry` を直接操作しない。

#### Scenario: Driver が registry を所有

- **WHEN** `RemoteDriver` のフィールドを検査する
- **THEN** `AssociationRegistry` を `Arc` / `ArcShared` / `SpinSyncMutex` でラップせず直接保持している

#### Scenario: adapter からの registry 直接操作の不在

- **WHEN** `modules/remote-adaptor-std/src/std` 配下を検査する
- **THEN** `AssociationRegistry` の状態変更メソッド（`enqueue` / `next_outbound` / `accept_handshake_*` 等）を直接呼んでいる箇所が存在しない（Driver 経由のみ）

### Requirement: 戻り値の握りつぶし禁止

Driver 内で `RemoteEventSink::push`、`RemoteTransport::*`、`Codec::*`、`Timer::*` の戻り値（`Result` または `#[must_use]`）を `let _ = ...` で握りつぶしてはならない（MUST NOT）。

#### Scenario: 戻り値の明示的扱い

- **WHEN** Driver の実装ソースを検査する
- **THEN** `let _ = ...` による `Result` 握りつぶしが存在しない
- **AND** 失敗は `?` で伝播するか、`match` で観測可能な経路（log / metric / outcome）に分岐する
