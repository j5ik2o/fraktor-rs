## Why

`remote-core` は Pekko Artery 互換の状態機械、Port trait、wire model を no_std で備えるが、現在は駆動の主導権が `remote-adaptor-std` 側の tokio task 群に握られている。`outbound_loop` が 1ms ポーリングで `Association::next_outbound` を回し、`inbound_dispatch` が `accept_handshake_*` を直接呼び、`HandshakeDriver` が timeout を駆動する。`AssociationEffect::StartHandshake` は adaptor 側で無視されており、core が表明した意図が adapter に伝わらない設計欠陥が残っている。

これにより以下の問題が生じる。

- 組み込み / WASM 等で std を使えない環境では tokio task 群を移植する必要があり、core 主導という設計意図が機能しない。
- `remote-core` には 604 行の `RemoteInstrument` / `RemotingFlightRecorder` が定義されているが、`Association` から一度も呼ばれておらず、配信失敗・handshake 進捗・quarantine が観測不能である。
- Pekko Artery が保証する system message の飢餓回避（control / ordinary queue 分離）と双方向 backpressure（watermark）が現状の優先度フィールドだけでは満たされない。
- `Remote` から `Codec`、`RemoteTransport`、association registry を駆動する経路が暗黙で、Driver lifecycle（起動 / 停止 / 再起動）が文書化されていない。

正式リリース前の今、Port & Adapter の純度を上げ、駆動主導権を core 側に反転する。adapter は I/O と event 通知だけを担当する形に退化させる。

## What Changes

### 1. `RemoteDriver` を core に新設し、駆動主導権を反転する

`modules/remote-core/src/core/driver/` を新設し、以下を提供する。

- `RemoteDriver`：`AssociationRegistry` と `RemoteTransport` 参照を保持し、`async fn run<S: RemoteEventSource>(self, source: S) -> RemoteDriverOutcome` を提供する駆動本体
- `RemoteEvent`：closed enum。`InboundFrameReceived { authority, frame }`、`OutboundFrameAcked { authority, sequence }`、`HandshakeTimerFired { authority, generation }`、`ConnectionLost { authority, cause }`、`TransportShutdown` などを含む
- `RemoteEventSource`：`async fn recv(&mut self) -> Option<RemoteEvent>` を持つ Port
- `RemoteEventSink`：`fn push(&self, event: RemoteEvent) -> Result<(), RemoteEventDispatchError>` を持つ Port（adapter → core 方向）
- `Timer`：`async fn schedule(&self, delay_ms: u64) -> RemoteEvent` を返す Port、または expiry 時に sink へ push する Port のいずれか（design.md で確定）
- `RemoteDriverHandle`：start / shutdown / 終了結果取得を提供する lifecycle 制御

Driver は `RemoteEvent` を受けて `Association` の `&mut self` メソッドに dispatch し、得られた `AssociationEffect` を実行する。`AssociationEffect::StartHandshake` を復活させ、Driver から `RemoteTransport::initiate_handshake` を呼ぶ。

### 2. `RemoteInstrument` をジェネリクス + composite 合成で `Remote` に配線する

`Remote` を `pub struct Remote<I: RemoteInstrument = NoopInstrument>` 化し、`I` をジェネリクスで保持する。`Arc<dyn RemoteInstrument>` を hot path で dispatch する形は採らない（dyn 越し dispatch を `on_send` / `on_receive` 全エンベロープに掛けるコストを回避するため）。

複数 instrument を合成するために `RemoteInstrumentTuple` 型クラス（または `(I1, I2, I3)` の trait 実装）を提供し、ユーザーは型レベルで合成する。`RemotingFlightRecorder` も `RemoteInstrument` 実装の 1 つとして合成可能にする。

`Remote::associate` / `accept_handshake_*` / `quarantine` / `next_outbound` / inbound dispatch / `apply_backpressure` から instrument の対応 method を呼ぶ経路を確定する（呼び出し点は `remote-core-association-state-machine` capability で要件化）。

### 3. system message 飢餓回避は既存の system / user 2 キュー分離で維持する

`Association::SendQueue` は既存仕様（`remote-core-association-state-machine` capability）で system priority と user priority の 2 キュー分離を持ち、system 優先で取り出す挙動が規定されている。本 change ではこの構造を維持し、Pekko Artery の Control / Ordinary 分離と同等の飢餓回避を継続する。

Large message queue は本 change では追加しない（frame size 上限と分割再送ロジックを伴うため独立 change で扱う）。

`Association` には watermark 連動の前提として、`SendQueue` の system + user 合計長を返すクエリ（`total_outbound_len(&self) -> usize`）を追加する（deferred queue は含めない）。

### 4. 双方向 watermark backpressure を導入する

各 `Association` に outbound queue の `high_watermark` / `low_watermark` を `RemoteConfig` から導入し、queue 長が high を超えると `apply_backpressure(BackpressureSignal::Engaged)` を発火、low を下回ると `apply_backpressure(BackpressureSignal::Released)` を発火する。signal は `RemotingFlightRecorder::record_backpressure` 経由で観測可能とする。

inbound 側は `RemoteEvent::InboundFrameReceived` 処理中に Driver が ack/nack を Transport に返す経路で表現し、ack 遅延（応用 ack window）は本 change では導入しない（過度な複雑化を避けるため、対応は別 change で扱う）。

### 5. Driver lifecycle と Codec 経路を明文化する

- `RemoteDriverHandle::shutdown(reason)` 経由で Driver を停止し、停止理由は `RemoteDriverOutcome::Shutdown { reason }` で受け取る。
- `RemoteEventSource` が `None` を返したら Driver は `RemoteDriverOutcome::SourceExhausted` で終了する。
- panic / 復帰不能エラーは `RemoteDriverOutcome::Aborted { error }` で表現する。
- adapter 側は `RemoteEvent::InboundFrameReceived` で raw bytes を sink に push し、Driver は `Codec::decode` で `InboundEnvelope` に復号してから `Association::accept_handshake_*` / inbound dispatch に渡す。outbound 側も Driver が `Codec::encode` で raw bytes 化してから `RemoteTransport::send_frame` を呼ぶ。

### 6. adaptor task を I/O ワーカーに退化させる

`remote-adaptor-std` の以下を削除する。

- `outbound_loop.rs`（1ms ポーリングで `next_outbound` を回す tokio task）
- `handshake_driver.rs`（timeout を tokio sleep で駆動する task）

`inbound_dispatch.rs` は `RemoteEvent::InboundFrameReceived` を sink に push する I/O ワーカーに退化させ、`accept_handshake_*` を直接呼ぶ責務を Driver に移す。

`effect_application.rs` の `StartHandshake` ignore 分岐を削除する（Driver が処理するため adapter では unreachable）。

`RemotingExtensionInstaller` から `RemoteDriver` を tokio task として spawn し、`RemoteDriverHandle` を保持する起動経路を追加する。停止時は `shutdown()` を呼び、Driver task の join を待つ。

### 7. timer Port と adapter 実装を追加する

core 側に `Timer` Port を定義し、adapter 側で tokio ベースの `TokioTimer` 実装を提供する。Driver は handshake timeout / heartbeat 周期 / quarantine timer を `Timer` 経由で取得する。

## Capabilities

### New Capabilities

- **`remote-core-driver`**
  - `RemoteDriver` が core 側で association の駆動主導権を持つ
  - `RemoteEventSource` / `RemoteEventSink` / `Timer` / `Codec` を Port として要求する
  - `AssociationEffect::StartHandshake` を実行する
  - `RemoteDriverHandle` で start / shutdown / 終了結果を制御する
  - Control / Ordinary queue 分離と watermark backpressure は Driver の outbound 駆動契約として表現される

### Modified Capabilities

- **`remote-core-instrument`**
  - `Remote` がジェネリクス `I: RemoteInstrument` で instrument を保持する
  - tuple ベースの composite 合成を提供する
  - `Arc<dyn RemoteInstrument>` を hot path で経由しない

- **`remote-core-association-state-machine`**
  - instrument hook を `associate` / `handshake_accepted` / `handshake_timed_out` / `quarantine` / `next_outbound` / inbound dispatch / `apply_backpressure` から呼ぶ
  - 既存の system / user 2 キュー分離は維持する
  - watermark 連動のため `total_outbound_len(&self)` クエリを追加する
  - handshake generation を `Association` に持たせ、古い timeout の発火を Driver 側で識別可能にする
  - `AssociationEffect::StartHandshake` を core / Driver 間契約として復活させる（adapter 無視を禁止する）

- **`remote-adaptor-std-runtime`**
  - `outbound_loop` / `handshake_driver` を削除する
  - `inbound_dispatch` は `RemoteEvent` を sink に push する I/O ワーカーに退化する
  - `RemoteDriver` を tokio task として spawn する起動経路を提供する
  - `tokio::sync::mpsc` ベースの `RemoteEventSource` / `RemoteEventSink` 実装を提供する
  - tokio ベースの `Timer` Port 実装を提供する

## Impact

**影響を受けるコード:**

- `modules/remote-core/src/core/driver/`（新規）
- `modules/remote-core/src/core/extension/remote.rs`（instrument ジェネリクス化、Driver 配線）
- `modules/remote-core/src/core/association/base.rs`（instrument 呼び出し点、queue 分離、watermark 連動）
- `modules/remote-core/src/core/association/registry.rs`（instrument 参照経路、queue 分離追従）
- `modules/remote-core/src/core/association/effect.rs`（`StartHandshake` 復活）
- `modules/remote-core/src/core/instrument/`（composite 合成 trait 実装）
- `modules/remote-core/src/core/transport/remote_transport.rs`（initiate_handshake / send_frame 整理）
- `modules/remote-core/src/core/wire/codec.rs`（Driver からの呼び出し前提を明示）
- `modules/remote-adaptor-std/src/std/outbound_loop.rs`（削除）
- `modules/remote-adaptor-std/src/std/handshake_driver.rs`（削除）
- `modules/remote-adaptor-std/src/std/inbound_dispatch.rs`（I/O ワーカーへ縮退）
- `modules/remote-adaptor-std/src/std/effect_application.rs`（`StartHandshake` ignore 削除）
- `modules/remote-adaptor-std/src/std/event_source.rs`（新規、tokio mpsc 実装）
- `modules/remote-adaptor-std/src/std/timer.rs`（新規、tokio timer 実装）
- `modules/remote-adaptor-std/src/std/extension_installer.rs`（Driver spawn 経路追加）

**公開 API 影響:**

- `Remote` がジェネリクス `Remote<I>` になる破壊的変更。`I = NoopInstrument` をデフォルト型で吸収する。
- `AssociationEffect::StartHandshake` の意味論が「adapter が無視」から「Driver が実行」へ変わる。
- adapter 側の `outbound_loop` / `handshake_driver` 公開関数は削除される。これは前 change `hide-remote-adaptor-runtime-internals` で internal 化済みのため外部 API 影響は無い。
- `RemoteEvent`、`RemoteEventSource`、`RemoteEventSink`、`Timer`、`RemoteDriver`、`RemoteDriverHandle` を新規 public 型として追加する。
- `RemoteConfig` に `outbound_high_watermark` / `outbound_low_watermark` を追加する。

**挙動影響:**

- 1ms ポーリングが消え、event 駆動になる。outbound throughput と CPU 消費が改善する。
- system message が ordinary message に飢餓されないことが保証される。
- queue が watermark を超えると backpressure signal が発火し、計測可能になる。
- handshake / quarantine / send / receive の全イベントが instrument に通知される。
- `RemoteDriver` 停止時の動作が `RemoteDriverOutcome` で明示される。

## Non-goals

- payload serialization の完成、wire protocol の再設計
- large message queue の追加（control / ordinary 分離のみ）
- inbound 側 ack window / 動的 receive buffer の調整
- cluster adaptor、persistence adaptor の駆動見直し
- failure detector の駆動経路変更（heartbeat は別 change で扱う）
- `Codec<T>` trait 自体のシグネチャ変更
- 後方互換 shim、deprecated alias、旧 API 残置
