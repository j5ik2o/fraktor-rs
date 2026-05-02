## REMOVED Requirements

### Requirement: outbound loop (送信 tokio task)

**理由:** `RemoteDriver` が core 側で outbound 駆動を担当するため、adapter 側の `outbound_loop` は不要となる。Driver が `Association::next_outbound()` を呼び、`Codec::encode` を経て `RemoteTransport` に送信する経路に置き換えられる。

### Requirement: handshake driver (タイムアウト監視)

**理由:** `RemoteDriver` が `Timer` Port 経由で `RemoteEvent::HandshakeTimerFired` を予約・受信するため、adapter 側の独立した tokio sleep task は不要となる。

## MODIFIED Requirements

### Requirement: inbound dispatch (受信 I/O ワーカー)

adapter 側に受信 tokio task が定義され、TCP から受信した frame を `RemoteEvent::InboundFrameReceived { authority, frame }` として `RemoteEventSink` に push する SHALL。`Association` の `handshake_accepted` 等を **直接呼んではならない**（MUST NOT）— state machine への反映は core 側の `RemoteDriver` が担当する。

#### Scenario: 受信 frame の sink push

- **WHEN** 受信 loop が TCP frame を受信する
- **THEN** `RemoteEvent::InboundFrameReceived { authority, frame }` を構築する
- **AND** `RemoteEventSink::push(event)` を呼び、`Result<(), RemoteEventDispatchError>` を観測可能に扱う（`?` または `match`）

#### Scenario: Association 直接呼び出しの不在

- **WHEN** `modules/remote-adaptor-std/src/std/inbound_dispatch.rs` または同等のソースを検査する
- **THEN** `Association::handshake_accepted` / `accept_handshake_request` / `accept_handshake_response` 等の core state 遷移メソッドを直接呼ぶ箇所が存在しない

#### Scenario: monotonic 時刻入力の不要化

- **WHEN** inbound I/O ワーカーが core に時刻を渡すかどうかを検査する
- **THEN** I/O ワーカーは時刻を渡さない（時刻は Driver 側で `Timer` Port または adapter 提供の `now_provider` 経由で取得する）
- **AND** wall clock の混入は発生しない

## ADDED Requirements

### Requirement: RemoteDriver を tokio task として spawn する起動経路

`RemotingExtensionInstaller` または同等の installer は `RemoteDriver` を tokio task として spawn し、`RemoteDriverHandle` を保持する SHALL。actor system の停止時に `RemoteDriverHandle::shutdown` を呼び、Driver task の `outcome().await` を待つ。

#### Scenario: Driver の spawn

- **WHEN** `RemotingExtensionInstaller` が remote 拡張を actor system に登録する
- **THEN** 内部で `tokio::spawn` または同等の経路で `RemoteDriver::run(source)` を起動する
- **AND** 起動結果として `RemoteDriverHandle` を保持する

#### Scenario: Driver の正常停止

- **WHEN** actor system が停止フェーズに入る
- **THEN** installer は `RemoteDriverHandle::shutdown(reason)` を呼ぶ
- **AND** Driver task の `outcome().await` を待ち、`RemoteDriverOutcome::Shutdown { reason }` を確認する

#### Scenario: Driver の異常終了の観測

- **WHEN** Driver が `RemoteDriverOutcome::Aborted { error }` を返した
- **THEN** installer は error を log に記録し、actor system の error path に伝播する
- **AND** `let _ = ...` による無言握りつぶしは存在しない

### Requirement: tokio mpsc ベースの RemoteEventSource / RemoteEventSink

adapter 側に `tokio::sync::mpsc` ベースの `RemoteEventSource` / `RemoteEventSink` 実装が定義される SHALL。

#### Scenario: Source 実装の存在

- **WHEN** `modules/remote-adaptor-std/src/std/event_source.rs` を読む
- **THEN** `pub struct TokioMpscEventSource` または同等の型が定義され、`impl RemoteEventSource` を持つ
- **AND** 内部で `tokio::sync::mpsc::UnboundedReceiver<RemoteEvent>` または `bounded receiver` を保持する

#### Scenario: Sink 実装の存在

- **WHEN** `modules/remote-adaptor-std/src/std/event_sink.rs` または同等のソースを読む
- **THEN** `pub struct TokioMpscEventSink` または同等の型が定義され、`impl RemoteEventSink` を持つ
- **AND** 内部で `tokio::sync::mpsc::UnboundedSender<RemoteEvent>` または `bounded sender` を保持する

#### Scenario: bounded / unbounded の選択

- **WHEN** Source / Sink 実装の channel 形式を検査する
- **THEN** bounded（背圧あり）か unbounded（背圧なし）かが docstring に明記されている
- **AND** 既定では unbounded を選択しない場合、`RemoteConfig` から channel capacity を読む

### Requirement: tokio ベースの Timer 実装

adapter 側に `tokio::time` ベースの `Timer` Port 実装が定義される SHALL。

#### Scenario: TokioTimer の存在

- **WHEN** `modules/remote-adaptor-std/src/std/timer.rs` を読む
- **THEN** `pub struct TokioTimer` または同等の型が定義され、`impl Timer` を持つ
- **AND** 内部で `tokio::time::sleep_until` または `tokio::time::Sleep` を使う

#### Scenario: schedule の発火経路

- **WHEN** `TokioTimer::schedule(delay, event)` を呼ぶ
- **THEN** 別 task として sleep を起動し、満了時に `RemoteEventSink::push(event)` を呼ぶ
- **AND** 戻り値の `TimerToken` は `cancel` で sleep task をキャンセル可能とする

#### Scenario: cancel の冪等性

- **WHEN** 同じ `TimerToken` に対して `cancel` を 2 回呼ぶ
- **THEN** 2 回目は no-op として安全に動作する

### Requirement: 効果適用から StartHandshake 分岐を削除する

adapter 側の `effect_application::apply_effects_in_place`（または相当箇所）から `AssociationEffect::StartHandshake` の dispatch 分岐を削除する SHALL。Driver 側が `RemoteTransport::initiate_handshake` を呼ぶため、adapter 側で同 effect を扱う必要がない。

#### Scenario: StartHandshake 分岐の不在

- **WHEN** `modules/remote-adaptor-std/src/std/effect_application.rs` を検査する
- **THEN** `AssociationEffect::StartHandshake { .. } =>` 分岐が存在しない
- **AND** unreachable! の使用も存在しない（Driver がすべて処理するため adapter のこの経路を通らない）

#### Scenario: 残存 effect の adapter 処理

- **WHEN** adapter 側で扱うべき effect 種別を検査する
- **THEN** `SendEnvelopes`、`DiscardEnvelopes`、`PublishLifecycle` 等の I/O 直結 effect のみが adapter 側で扱われ、状態遷移を伴う effect は Driver 側に集約される
