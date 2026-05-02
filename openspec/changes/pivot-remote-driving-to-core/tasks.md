## 1. instrument 配線基盤を core 側に整える

- [ ] 1.1 `modules/remote-core/src/core/instrument/noop_instrument.rs` を新設し、`pub struct NoopInstrument;` と `impl RemoteInstrument for NoopInstrument` を追加する。
- [ ] 1.2 `modules/remote-core/src/core/instrument/composite.rs`（または `remote_instrument.rs` 配下）に tuple-based composite `impl RemoteInstrument for (A, B)` および `(A, B, C)` を追加する。
- [ ] 1.3 `modules/remote-core/src/core/extension/remote.rs` を `pub struct Remote<I: RemoteInstrument = NoopInstrument>` に変更し、ビルダ・コンストラクタ・既存 method の型注釈を `<I>` 対応にする。
- [ ] 1.4 `modules/remote-core/src/core/instrument/flight_recorder.rs` に `impl RemoteInstrument for RemotingFlightRecorder` を追加し、record 系メソッドを RemoteInstrument hook 経由でも発火可能にする。
- [ ] 1.5 instrument 単体 unit test を追加する。`Remote<NoopInstrument>` と `Remote<(RemotingFlightRecorder, MyMetrics)>` の両構築を確認し、tuple composite で順次 dispatch されることを検証する。

## 2. Association に instrument hook と watermark 用 query を追加

- [ ] 2.1 `Association::associate` のシグネチャに `instrument: &mut I` を追加し、`record_handshake(authority, HandshakePhase::Started, now_ms)` を内部から呼び出す。
- [ ] 2.2 `Association::handshake_accepted` に同様の instrument 引数を追加し、`record_handshake(.., HandshakePhase::Accepted, ..)` を呼び出す。
- [ ] 2.3 `Association::handshake_timed_out` に instrument 引数を追加し、`record_handshake(.., HandshakePhase::Rejected, ..)` を呼び出す。
- [ ] 2.4 `Association::quarantine` に instrument 引数を追加し、`record_quarantine(authority, reason, now_ms)` を呼び出す。
- [ ] 2.5 `Association::apply_backpressure` に instrument 引数を追加し、`record_backpressure(authority, signal, correlation_id, now_ms)` を呼び出す。
- [ ] 2.6 `Association::next_outbound` の戻り値経路（または直近の Driver 呼び出し点）で `on_send(envelope)` を発火する経路を確立する（呼び出し点は Driver 側 task と整合させる）。
- [ ] 2.7 inbound dispatch 経路で `on_receive(envelope)` を発火するための公開 method を Association に追加する（または既存 method に instrument 引数を渡す）。
- [ ] 2.8 `Association::total_outbound_len(&self) -> usize` を追加する（`SendQueue` の system + user 合計、deferred は含めない）。
- [ ] 2.9 `Association` に `handshake_generation: HandshakeGeneration` フィールドを追加し、`Handshaking` 状態に入るたびに +1 する。
- [ ] 2.10 上記変更の unit test を追加する（各 hook 呼び出し点で記録された FlightRecorder snapshot で順序を検証）。

## 3. core 側に Driver 関連 Port を追加

- [ ] 3.1 `modules/remote-core/src/core/driver/remote_event.rs` を新設し、`pub enum RemoteEvent` を closed enum で定義する（必要バリアントは spec 参照）。
- [ ] 3.2 `modules/remote-core/src/core/driver/remote_event_source.rs` を新設し、`pub trait RemoteEventSource` を定義する（`recv` の async 化方式は design.md の Open Questions 対応）。
- [ ] 3.3 `modules/remote-core/src/core/driver/remote_event_sink.rs` を新設し、`pub trait RemoteEventSink: Send + Sync` を定義する。
- [ ] 3.4 `modules/remote-core/src/core/driver/timer.rs` を新設し、`pub trait Timer: Send + Sync` と `pub struct TimerToken` を定義する。
- [ ] 3.5 `RemoteEventDispatchError`、`RemoteDriverError` を必要に応じ追加する。
- [ ] 3.6 `RemoteDriverOutcome` enum を定義する（`Shutdown` / `SourceExhausted` / `Aborted`）。

## 4. core 側に RemoteDriver を実装

- [ ] 4.1 `modules/remote-core/src/core/driver/remote_driver.rs` を新設し、`pub struct RemoteDriver<S, K, T, I, C>` のジェネリクス署名と field を定義する。
- [ ] 4.2 `RemoteDriver::run(self, source: S) -> RemoteDriverOutcome` の async ループ skeleton を実装する（event match の dispatch 表）。
- [ ] 4.3 `RemoteEvent::InboundFrameReceived` 処理を実装する（`Codec::decode` → Association inbound dispatch → instrument `on_receive`）。
- [ ] 4.4 `RemoteEvent::HandshakeTimerFired` 処理を実装する（generation 検査 → `Association::handshake_timed_out` 呼び出し）。
- [ ] 4.5 `RemoteEvent::QuarantineTimerFired` 処理を実装する。
- [ ] 4.6 `RemoteEvent::ConnectionLost` 処理を実装する（再接続判断と `Association::recover` 呼び出し）。
- [ ] 4.7 `RemoteEvent::TransportShutdown` 処理を実装する（`RemoteDriverOutcome::Shutdown { reason }` で run を return）。
- [ ] 4.8 outbound 駆動 helper（`Association::next_outbound` → `Codec::encode` → `RemoteTransport::send`）を実装する。
- [ ] 4.9 `AssociationEffect::StartHandshake` 実行経路（`RemoteTransport::initiate_handshake` + `Timer::schedule(handshake_timeout, HandshakeTimerFired)`）を実装する。
- [ ] 4.10 watermark 連動 backpressure 発火（`total_outbound_len` を high / low と比較し `apply_backpressure` を呼ぶ）を outbound helper に組み込む。
- [ ] 4.11 `RemoteDriverHandle` 型と `shutdown(reason) -> Result<(), RemoteEventDispatchError>` を実装する。
- [ ] 4.12 Driver の unit test を追加する（fake `RemoteTransport`、fake `Timer`、in-memory source/sink で event 列を流して期待状態遷移を検証）。

## 5. core 側に RemoteConfig::outbound_high_watermark / low_watermark を追加

- [ ] 5.1 `RemoteConfig` に `outbound_high_watermark: usize` と `outbound_low_watermark: usize` を追加する（既定値は high=1024, low=512）。
- [ ] 5.2 `outbound_low_watermark < outbound_high_watermark` の validation を `RemoteConfig::validate`（または builder）で実装する。
- [ ] 5.3 設定読取の unit test を追加する。

## 6. core 側で AssociationEffect::StartHandshake のセマンティクス整合

- [ ] 6.1 `AssociationEffect::StartHandshake` の rustdoc を更新し、「Driver が `RemoteTransport::initiate_handshake` を呼ぶ責務」を明示する。
- [ ] 6.2 既存 unit test で `recover(Some(endpoint), now)` が `StartHandshake` を返すことを確認する（既存仕様維持）。

## 7. adapter 側で I/O ワーカー化と Driver spawn 経路を追加

- [ ] 7.1 `modules/remote-adaptor-std/src/std/inbound_dispatch.rs` を I/O ワーカーに変更し、TCP frame 受信後に `RemoteEvent::InboundFrameReceived` を sink に push するだけの処理にする。`Association::handshake_accepted` 等の直接呼び出しを削除する。
- [ ] 7.2 `modules/remote-adaptor-std/src/std/event_source.rs` を新設し、`TokioMpscEventSource: RemoteEventSource` を実装する（`tokio::sync::mpsc::UnboundedReceiver` または bounded receiver）。
- [ ] 7.3 `modules/remote-adaptor-std/src/std/event_sink.rs` を新設し、`TokioMpscEventSink: RemoteEventSink` を実装する。
- [ ] 7.4 `modules/remote-adaptor-std/src/std/timer.rs` を新設し、`TokioTimer: Timer` を実装する（`tokio::time::sleep_until` ベース、cancel 冪等性確保）。
- [ ] 7.5 `RemotingExtensionInstaller` に `RemoteDriver` を `tokio::spawn` で起動する経路を追加し、`RemoteDriverHandle` を保持する。
- [ ] 7.6 actor system 停止時に `RemoteDriverHandle::shutdown` を呼び、Driver の `outcome().await` を待つ経路を追加する。`RemoteDriverOutcome::Aborted` 時は error を log と error path に伝播する（`let _` で握りつぶさない）。
- [ ] 7.7 `modules/remote-adaptor-std/src/std/effect_application.rs` から `AssociationEffect::StartHandshake` の dispatch 分岐を削除する。

## 8. adapter 側で旧 task を削除

- [ ] 8.1 `modules/remote-adaptor-std/src/std/outbound_loop.rs` を削除し、`mod` 宣言と `pub(crate) use` 経路を整理する。
- [ ] 8.2 `modules/remote-adaptor-std/src/std/handshake_driver.rs` を削除し、`mod` 宣言と関連 import を整理する。
- [ ] 8.3 旧 task に依存していた helper（`reconnect_backoff_policy`、`restart_counter` 等）の所属を見直し、Driver で使うものは `modules/remote-core/src/core/driver/` または `core/policy/` に移動する。
- [ ] 8.4 削除後に残る dead code（unused import、unused fields）を整理する。

## 9. テスト

- [ ] 9.1 `rtk cargo test -p fraktor-remote-core-rs` を実行し、green を確認する。
- [ ] 9.2 `rtk cargo test -p fraktor-remote-adaptor-std-rs` を実行し、green を確認する。
- [ ] 9.3 `rtk cargo test -p fraktor-cluster-adaptor-std-rs` を実行し、依存先の green を確認する。
- [ ] 9.4 handshake / quarantine / watermark backpressure / instrument 通知の integration test を追加または更新する（public API 経由で Driver を起動して検証）。
- [ ] 9.5 showcase（`showcases/std/remote_lifecycle/` 等）が新 API で起動することを確認する。

## 10. 検証

- [ ] 10.1 dylint（mod-file、module-wiring、type-per-file、tests-location、use-placement、rustdoc、cfg-std-forbid、ambiguous-suffix）を `rtk cargo clippy` 系で確認する。
- [ ] 10.2 `rtk ./scripts/ci-check.sh ai all` を最後まで完了させる。
