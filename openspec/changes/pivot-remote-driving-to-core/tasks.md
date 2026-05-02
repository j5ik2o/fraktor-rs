## 1. instrument 配線基盤を core 側に整える（dyn dispatch 採用、純増ゼロ方針）

- [ ] 1.1 `modules/remote-core/src/core/instrument/noop_instrument.rs`（または既存ファイル）に `pub(crate) struct NoopInstrument;` と `impl RemoteInstrument for NoopInstrument` を追加する（外部公開しない）。すべての method 本体は空。**`pub` での公開は禁止**（`pub(crate)` 限定）。
- [ ] 1.2 `modules/remote-core/src/core/extension/remote.rs` の `Remote` に `instrument: alloc::boxed::Box<dyn RemoteInstrument + Send>` フィールドを追加する。**`Remote` に型パラメータ `<I>` を導入してはならない**。
- [ ] 1.3 `Remote::new(transport, config, event_publisher)` を更新し、内部で `instrument: Box::new(NoopInstrument)` を割り当てる。既存呼出シグネチャは変更なし（フィールド追加のみのため非破壊）。
- [ ] 1.4 `Remote::with_instrument(transport, config, event_publisher, instrument: Box<dyn RemoteInstrument + Send>) -> Self` を新規 public API として追加する。
- [ ] 1.5 `Remote::set_instrument(&mut self, instrument: Box<dyn RemoteInstrument + Send>)` を新規 public API として追加し、rustdoc に「`run` 進行中に呼ばないこと」を明示する。
- [ ] 1.6 `modules/remote-core/src/core/instrument/flight_recorder.rs` に `impl RemoteInstrument for RemotingFlightRecorder` を追加し、record 系メソッドを RemoteInstrument hook 経由でも発火可能にする。
- [ ] 1.7 instrument 単体 unit test を追加する。`Remote::new` 既定（NoopInstrument）と `Remote::with_instrument(.., Box::new(RemotingFlightRecorder::new(..)))` の両構築を確認し、event loop で hook が ring buffer に届くことを検証する。
- [ ] 1.8 **tuple composite と `() impl` は追加しない** ことを確認する。`grep -n 'impl<.*> RemoteInstrument for (' modules/remote-core/src/` および `grep -n 'impl RemoteInstrument for ()' modules/remote-core/src/` の出力が空であること。

## 2. Association に instrument hook と watermark 用 query を追加（newtype を作らない）

- [ ] 2.1 `Association::associate` のシグネチャに `instrument: &mut dyn RemoteInstrument` を追加し、`record_handshake(authority, HandshakePhase::Started, now_ms)` を内部から呼び出す。**型パラメータ `<I>` は導入しない**。
- [ ] 2.2 `Association::handshake_accepted` に同様の `instrument: &mut dyn RemoteInstrument` 引数を追加し、`record_handshake(.., HandshakePhase::Accepted, ..)` を呼び出す。
- [ ] 2.3 `Association::handshake_timed_out` に `instrument: &mut dyn RemoteInstrument` 引数を追加し、`record_handshake(.., HandshakePhase::Rejected, ..)` を呼び出す。
- [ ] 2.4 `Association::quarantine` に `instrument: &mut dyn RemoteInstrument` 引数を追加し、`record_quarantine(authority, reason, now_ms)` を呼び出す。
- [ ] 2.5 `Association::apply_backpressure` に `instrument: &mut dyn RemoteInstrument` 引数を追加し、`record_backpressure(authority, signal, correlation_id, now_ms)` を呼び出す（既存 `BackpressureSignal::Apply` / `Release` をそのまま流用、新 variant 追加なし）。
- [ ] 2.6 `Association::next_outbound` の戻り値経路（または直近の `Remote::run` 呼び出し点）で `on_send(envelope)` を発火する経路を確立する。
- [ ] 2.7 inbound dispatch 経路で `on_receive(envelope)` を発火するための公開 method を Association に追加する（または既存 method に instrument 引数を渡す）。
- [ ] 2.8 `Association::total_outbound_len(&self) -> usize` を追加する（`SendQueue` の system + user 合計、deferred は含めない）。
- [ ] 2.9 `Association` に `handshake_generation: u64` フィールドを追加し、`Handshaking` 状態に入るたびに `wrapping_add(1)` で +1 する（**`HandshakeGeneration` newtype は新設しない**）。
- [ ] 2.10 `AssociationEffect::StartHandshake` を `{ authority: TransportEndpoint, timeout: core::time::Duration, generation: u64 }` に拡張し、rustdoc に「`Remote::run` が実行する責務」と「adapter は generation 付き timer を確保する責務」を明示する。
- [ ] 2.11 上記変更の unit test を追加する（各 hook 呼び出し点で記録された FlightRecorder snapshot で順序を検証）。

## 3. core 側に RemoteEvent / RemoteEventSource を追加（純増 2 個に抑制）

- [ ] 3.1 `modules/remote-core/src/core/extension/remote_event.rs` を新設し、`pub enum RemoteEvent` を closed enum で定義する（`InboundFrameReceived` / `OutboundFrameAcked` / `HandshakeTimerFired { generation: u64 }` / `QuarantineTimerFired` / `ConnectionLost` / `TransportShutdown` / `BackpressureCleared`）。
- [ ] 3.2 `modules/remote-core/src/core/extension/remote_event_source.rs` を新設し、`pub trait RemoteEventSource: Send` と `fn recv(&mut self) -> impl Future<Output = Option<RemoteEvent>> + Send + '_` を定義する。
- [ ] 3.3 `modules/remote-core/src/core/extension.rs`（または `mod.rs`）から `RemoteEvent` / `RemoteEventSource` を `pub use` 経由で公開する。
- [ ] 3.4 **`RemoteEventSink` / `Timer` / `RemoteDriver*` 系の trait・型は新設しないことを確認** する（dylint module-wiring と code review で担保）。

## 4. Remote::run を inherent method として実装

- [ ] 4.1 `modules/remote-core/src/core/extension/remote.rs` に `impl Remote` で `pub async fn run<S: RemoteEventSource>(&mut self, source: &mut S) -> Result<(), RemotingError>` の skeleton を追加する。`Remote` 自体には型パラメータ `<I>` を持たせない（instrument は `Box<dyn RemoteInstrument + Send>` フィールド経由）。ループ内では `let instrument: &mut dyn RemoteInstrument = &mut *self.instrument;` で local 借用を作る。
- [ ] 4.2 `RemoteEvent::InboundFrameReceived` 処理を実装する（`Codec::decode` → Association inbound dispatch → instrument `on_receive`）。
- [ ] 4.3 `RemoteEvent::HandshakeTimerFired { generation }` 処理を実装する（`Association.handshake_generation` と `!=` で比較し、不一致時は event を破棄。一致時のみ `Association::handshake_timed_out` を呼ぶ。`>` / `<` 比較は使わない — `wrapping_add` の wrap で stale 判定が漏れないようにする）。
- [ ] 4.3.1 wrap 境界の unit test を追加する（`handshake_generation = u64::MAX` → 次回 `Handshaking` で `0` になり、古い `g_event = u64::MAX` の `HandshakeTimerFired` を受信した際に `!=` 判定で正しく破棄されること）。
- [ ] 4.4 `RemoteEvent::QuarantineTimerFired` 処理を実装する。
- [ ] 4.5 `RemoteEvent::ConnectionLost` 処理を実装する（再接続判断と `Association::recover` 呼び出し）。
- [ ] 4.6 `RemoteEvent::TransportShutdown` で `Ok(())` を返してループ終了する。
- [ ] 4.7 source 枯渇（`recv` が `None`）で `Ok(())` を返してループ終了する。
- [ ] 4.8 outbound 駆動 helper（`Association::next_outbound` → `Codec::encode` → `RemoteTransport::send`）を実装する。
- [ ] 4.9 `AssociationEffect::StartHandshake { authority, timeout, generation }` 実行経路（`RemoteTransport` 経由で handshake request 送出。timer 確保は adapter 側責務）を実装する。
- [ ] 4.10 watermark 連動 backpressure 発火（`total_outbound_len` を high / low と比較し `apply_backpressure(Apply | Release)` を呼ぶ）を outbound helper に組み込む。
- [ ] 4.11 復帰不能エラー時に `Err(RemotingError::TransportUnavailable)` を返す経路を実装する（`?` 伝播、`let _` 握りつぶし禁止）。
- [ ] 4.12 `Remote::run` の unit test を追加する（fake `RemoteTransport`、in-memory `RemoteEventSource` で event 列を流して期待状態遷移を検証）。

## 5. RemoteConfig に watermark 設定を追加

- [ ] 5.1 `RemoteConfig` に `outbound_high_watermark: usize` と `outbound_low_watermark: usize` を追加する（既定値は high=1024, low=512）。
- [ ] 5.2 `outbound_low_watermark < outbound_high_watermark` の validation を `RemoteConfig::validate`（または builder）で実装する。
- [ ] 5.3 設定読取の unit test を追加する。

## 6. AssociationEffect::StartHandshake のセマンティクス整合

- [ ] 6.1 `AssociationEffect::StartHandshake` の rustdoc を更新し、「`Remote::run` が `RemoteTransport` 経由で handshake request を送出する責務」「adapter は generation 付き timer を確保する責務」を明示する。
- [ ] 6.2 既存 unit test で `recover(Some(endpoint), now)` および `associate(...)` が拡張後の `StartHandshake { authority, timeout, generation }` を返すことを確認する（既存仕様維持 + generation 追加）。

## 7. adapter 側で I/O ワーカー化と Remote::run spawn 経路を追加

- [ ] 7.1 `modules/remote-adaptor-std/src/std/inbound_dispatch.rs` を I/O ワーカーに変更し、TCP frame 受信後に `RemoteEvent::InboundFrameReceived` を adapter 内部 sender 経由で push するだけの処理にする。`Association::handshake_accepted` 等の直接呼び出しを削除する。
- [ ] 7.2 `modules/remote-adaptor-std/src/std/tokio_remote_event_source.rs` を新設し、`TokioMpscRemoteEventSource: RemoteEventSource` を実装する（`tokio::sync::mpsc::Receiver<RemoteEvent>` を保持。bounded、capacity は `RemoteConfig` 経由）。
- [ ] 7.3 adapter 内部で `tokio::sync::mpsc::channel::<RemoteEvent>(capacity)` を生成し、`Sender` を I/O ワーカー / handshake timer task 群が clone して共有する経路を整備する（`RemoteEventSink` trait は core に追加しない）。
- [ ] 7.4 `RemotingExtensionInstaller` に `Remote::run(&mut source)` を `tokio::spawn` で起動する経路を追加し、`JoinHandle<Result<(), RemotingError>>` を保持する。
- [ ] 7.5 actor system 停止時に `Remoting::shutdown` 経由で adapter 内部 sender に `RemoteEvent::TransportShutdown` を push し、`Remote::run` task の `JoinHandle::await` を待つ経路を追加する。`Err(_)` 時は error を log と error path に伝播する（`let _` で握りつぶさない）。
- [ ] 7.6 `modules/remote-adaptor-std/src/std/effect_application.rs` から `AssociationEffect::StartHandshake` の dispatch 分岐を削除する。
- [ ] 7.7 handshake timer 確保経路を adapter 側に実装する。`AssociationEffect::StartHandshake { authority, timeout, generation }` を契機に `tokio::spawn(async move { tokio::time::sleep(timeout).await; sender.send(RemoteEvent::HandshakeTimerFired { authority, generation }).await; })` 相当のタスクを起動する（`Timer` trait は core に新設しない）。

## 8. adapter 側で旧 task を削除

- [ ] 8.1 `modules/remote-adaptor-std/src/std/outbound_loop.rs` を削除し、`mod` 宣言と `pub(crate) use` 経路を整理する。
- [ ] 8.2 `modules/remote-adaptor-std/src/std/handshake_driver.rs` を削除し、`mod` 宣言と関連 import を整理する（handshake timer 責務は task 7.7 に統合される）。
- [ ] 8.3 旧 task に依存していた helper（`reconnect_backoff_policy`、`restart_counter` 等）の所属を見直し、`Remote::run` で使うものは `modules/remote-core/src/core/extension/` または既存の core 側既存ファイルに移動する（新規 `core/driver/` ディレクトリは作らない）。
- [ ] 8.4 削除後に残る dead code（unused import、unused fields）を整理する。

## 9. 純増ゼロ検証

- [ ] 9.1 `git diff main..HEAD --stat` または同等で、新規追加された core 側公開型・公開 trait の数を数え、`RemoteEvent` enum と `RemoteEventSource` trait の **2 つだけ** であることを確認する。
- [ ] 9.2 公開禁止型・禁止 trait が core 側に追加されていないことを以下のクエリで確認する（出力が空であること）。
  - `grep -rn 'pub struct RemoteDriver\|pub trait Timer\b\|pub trait RemoteEventSink\|pub struct HandshakeGeneration\|pub struct TimerToken\|pub struct RemoteDriverHandle\|pub enum RemoteDriverOutcome' modules/remote-core/src/`
  - `grep -rn 'pub struct NoopInstrument' modules/remote-core/src/`（`NoopInstrument` は `pub(crate)` のみ許可）
  - `grep -rn 'impl<.*> RemoteInstrument for (' modules/remote-core/src/`（tuple composite 禁止）
  - `grep -rn 'impl RemoteInstrument for ()' modules/remote-core/src/`（`()` impl 禁止）
  - `grep -rn 'pub struct Remote<' modules/remote-core/src/core/extension/`（`Remote` ジェネリクス化禁止）
- [ ] 9.3 net file delta が `+1` 以下（`remote_event.rs` + `remote_event_source.rs` + `tokio_remote_event_source.rs` 追加 / `outbound_loop.rs` + `handshake_driver.rs` 削除）であることを確認する。

## 10. テスト

- [ ] 10.1 `rtk cargo test -p fraktor-remote-core-rs` を実行し、green を確認する。
- [ ] 10.2 `rtk cargo test -p fraktor-remote-adaptor-std-rs` を実行し、green を確認する。
- [ ] 10.3 `rtk cargo test -p fraktor-cluster-adaptor-std-rs` を実行し、依存先の green を確認する。
- [ ] 10.4 handshake / quarantine / watermark backpressure / instrument 通知 / handshake generation 破棄の integration test を追加または更新する（public API 経由で `Remote::run` を起動して検証）。
- [ ] 10.5 showcase（`showcases/std/remote_lifecycle/` 等）が新 API で起動することを確認する。

## 11. 検証

- [ ] 11.1 dylint（mod-file、module-wiring、type-per-file、tests-location、use-placement、rustdoc、cfg-std-forbid、ambiguous-suffix）を `rtk cargo clippy` 系で確認する。
- [ ] 11.2 `rtk ./scripts/ci-check.sh ai all` を最後まで完了させる。
