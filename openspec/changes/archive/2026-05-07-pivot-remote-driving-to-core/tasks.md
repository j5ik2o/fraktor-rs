## 1. instrument 配線基盤を core 側に整える（dyn dispatch 採用、純増ゼロ方針）

- [x] 1.1 `modules/remote-core/src/core/instrument/noop_instrument.rs` を新設し、`pub(crate) struct NoopInstrument;` と `impl RemoteInstrument for NoopInstrument` を追加する（1 file 1 type ルールに従い専用ファイルを作る、外部公開しない）。すべての method 本体は空。**`pub` での公開は禁止**（`pub(crate)` 限定）。
- [x] 1.2 `modules/remote-core/src/core/extension/remote.rs` の `Remote` に `instrument: alloc::boxed::Box<dyn RemoteInstrument + Send>` フィールドを追加する。**`Remote` に型パラメータ `<I>` を導入してはならない**。
- [x] 1.3 `Remote::new(transport, config, event_publisher)` を更新し、内部で `instrument: Box::new(NoopInstrument)` を割り当てる。既存呼出シグネチャは変更なし（フィールド追加のみのため非破壊）。
- [x] 1.4 `Remote::with_instrument(transport, config, event_publisher, instrument: Box<dyn RemoteInstrument + Send>) -> Self` を新規 public API として追加する。
- [x] 1.5 `Remote::set_instrument(&mut self, instrument: Box<dyn RemoteInstrument + Send>)` を新規 public API として追加し、rustdoc に「`run` 進行中に呼ばないこと」を明示する。
- [x] 1.6 `modules/remote-core/src/core/instrument/flight_recorder.rs` に `impl RemoteInstrument for RemotingFlightRecorder` を追加し、record 系メソッドを RemoteInstrument hook 経由でも発火可能にする。
- [x] 1.7 instrument 単体 unit test を追加する。`Remote::new` 既定（NoopInstrument）と `Remote::with_instrument(.., Box::new(RemotingFlightRecorder::new(..)))` の両構築を確認し、event loop で hook が ring buffer に届くことを検証する。
- [x] 1.8 **tuple composite と `() impl` は追加しない** ことを確認する。`grep -n 'impl<.*> RemoteInstrument for (' modules/remote-core/src/` および `grep -n 'impl RemoteInstrument for ()' modules/remote-core/src/` の出力が空であること。

## 2. Association に instrument hook と watermark 用 query を追加（newtype を作らない）

- [x] 2.1 `Association::associate` のシグネチャに `instrument: &mut dyn RemoteInstrument` を追加し、`record_handshake(authority, HandshakePhase::Started, now_ms)` を内部から呼び出す。**型パラメータ `<I>` は導入しない**。
- [x] 2.2 `Association::handshake_accepted` に同様の `instrument: &mut dyn RemoteInstrument` 引数を追加し、`record_handshake(.., HandshakePhase::Accepted, ..)` を呼び出す。
- [x] 2.3 `Association::handshake_timed_out` に `instrument: &mut dyn RemoteInstrument` 引数を追加し、`record_handshake(.., HandshakePhase::Rejected, ..)` を呼び出す。
- [x] 2.4 `Association::quarantine` に `instrument: &mut dyn RemoteInstrument` 引数を追加し、`record_quarantine(authority, reason, now_ms)` を呼び出す。
- [x] 2.5 `Association::apply_backpressure` に `instrument: &mut dyn RemoteInstrument` 引数を追加し、`record_backpressure(authority, signal, correlation_id, now_ms)` を呼び出す（既存 `BackpressureSignal::Apply` / `Release` をそのまま流用、新 variant 追加なし）。
- [x] 2.6 `Association::next_outbound` の戻り値経路（または直近の `Remote::handle_remote_event` 呼び出し点）で `on_send(envelope)` を発火する経路を確立する。
- [x] 2.7 inbound dispatch 経路で `on_receive(envelope)` を発火するための公開 method を Association に追加する（または既存 method に instrument 引数を渡す）。
- [x] 2.8 `Association::total_outbound_len(&self) -> usize` を追加する（`SendQueue` の system + user 合計、deferred は含めない）。
- [x] 2.9 `Association` に `handshake_generation: u64` フィールドを追加し、`Handshaking` 状態に入るたびに `wrapping_add(1)` で +1 する（**`HandshakeGeneration` newtype は新設しない**）。
- [x] 2.10 `AssociationEffect::StartHandshake` を `{ authority: TransportEndpoint, timeout: core::time::Duration, generation: u64 }` に拡張し、rustdoc に「`Remote::handle_remote_event`（`RemoteShared::run` の `with_write` 区間内）が実行する責務」と「adapter は generation 付き timer を確保する責務」を明示する。
- [x] 2.11 上記変更の unit test を追加する（各 hook 呼び出し点で記録された FlightRecorder snapshot で順序を検証）。
- [x] 2.12 既存の `Association::*_with_instrument` 併設 API（例: `associate_with_instrument` / `accept_handshake_request_with_instrument` / `accept_handshake_response_with_instrument` 等）を削除し、instrument 必須シグネチャ 1 本に統合する（`remote-core-association-state-machine` capability の「最終形では `*_with_instrument` 併設 API を残さない」要件に従う）。
  - `rtk grep -rn '_with_instrument' modules/remote-core/src/ modules/remote-adaptor-std/src/` で対象を洗い出す
  - 各 callers を instrument 必須シグネチャに更新
  - `rtk grep -rn '_with_instrument' modules/` の出力が空（または `pub(crate)` 内部 helper のみ）になることを確認

## 3. core 側に RemoteEvent / RemoteEventReceiver / RemoteTransport::schedule_handshake_timeout を追加

- [x] 3.1 `modules/remote-core/src/core/extension/remote_event.rs` を新設し、`pub enum RemoteEvent` を closed enum で定義する（**5 variant のみ**: `InboundFrameReceived { authority, frame: Vec<u8>, now_ms: u64 }` / `HandshakeTimerFired { authority, generation: u64, now_ms: u64 }` / `OutboundEnqueued { authority, envelope: Box<OutboundEnvelope>, now_ms: u64 }` / `ConnectionLost { authority, cause: ConnectionLostCause, now_ms: u64 }` / `TransportShutdown`）。**`OutboundFrameAcked` / `QuarantineTimerFired` / `BackpressureCleared` は本 change では追加しない**（必要時に別 change で variant 追加 + scheduling 経路 MODIFIED を一緒に行う）。
- [x] 3.2 `modules/remote-core/src/core/extension/remote_event_receiver.rs` を更新し、`pub trait RemoteEventReceiver` と `fn poll_recv(&mut self, cx: &mut core::task::Context<'_>) -> core::task::Poll<Option<RemoteEvent>>` を定義する。`Send` supertrait、`async fn recv`、`fn recv(..) -> impl Future` は禁止。
- [x] 3.3 `modules/remote-core/src/core/transport/remote_transport.rs` の `RemoteTransport` trait に `fn schedule_handshake_timeout(&mut self, authority: &TransportEndpoint, timeout: core::time::Duration, generation: u64) -> Result<(), TransportError>` を追加する（同期 method、`async fn` は使わない）。rustdoc に「adapter は満了時に monotonic 時刻を取得し、`RemoteEvent::HandshakeTimerFired { authority, generation, now_ms }` を adapter 内部 sender 経由で receiver に push する責務を持つ」を明記する。
- [x] 3.4 `modules/remote-core/src/core/extension.rs`（または `mod.rs`）から `RemoteEvent` / `RemoteEventReceiver` を `pub use` 経由で公開する。
- [x] 3.5 **`RemoteEventSink` / `Timer` / `RemoteDriver*` 系の trait・型は新設しないことを確認** する（dylint module-wiring と code review で担保）。

## 4. Remote::run / Remote::handle_remote_event と RemoteShared::run を実装

- [x] 4.0 既存の `Remote::run(self, ..)` consume 形を **`Remote::run(&mut self, ..) -> RemoteRunFuture<'_, S>` に変更して残す**。`Remote::run` は排他所有時の core event loop とし、poll 内部を `Remote::handle_remote_event` + `Remote::is_terminated` に分解する。`Remote::run` を削除して `RemoteShared::run` へ置き換えてはならない（MUST NOT）。`async fn` / `impl Future` 戻り値 / `Send` 境界は禁止。
- [x] 4.1 `modules/remote-core/src/core/extension/remote.rs` に `impl Remote` で `pub fn handle_remote_event(&mut self, event: RemoteEvent) -> Result<(), RemotingError>` の skeleton を追加する（**CQS Command、成功値は `()` のみ**、bool で停止判定を返さない）。`Remote` 自体には型パラメータ `<I>` を持たせない（instrument は `Box<dyn RemoteInstrument + Send>` フィールド経由）。event 処理で instrument と他 field を同時に扱う場合は、field 単位の split borrow が成立する helper へ切り出す。
- [x] 4.1.1 `Remote::is_terminated(&self) -> bool` Query method を追加する（`#[must_use]` 属性付き）。`lifecycle.is_terminated()` または `lifecycle.is_shutdown_requested()` のいずれかなら `true` を返す。`Remote::run` が直接、`RemoteShared::run` が per-event 後に `with_read` で確認してループ終了判定する。
- [x] 4.2 `RemoteEvent::InboundFrameReceived { authority, frame, now_ms }` 処理を `handle_remote_event` 内に実装する（core wire frame header の kind に応じて既存 `EnvelopeCodec` / `HandshakeCodec` / `ControlCodec` / `AckCodec` で decode → Association inbound dispatch → instrument `on_receive`）。decode 失敗は `RemotingError::CodecFailed` 等の caller が観測できる error に変換する。
- [x] 4.3 `RemoteEvent::HandshakeTimerFired { generation, now_ms }` 処理を実装する（`Association.handshake_generation` と `!=` で比較し、不一致時は event を破棄。一致時のみ `Association::handshake_timed_out(now_ms, ...)` を呼ぶ。`>` / `<` 比較は使わない — `wrapping_add` の wrap で stale 判定が漏れないようにする）。
- [x] 4.3.1 wrap 境界の unit test を追加する（`handshake_generation = u64::MAX` → 次回 `Handshaking` で `0` になり、古い `g_event = u64::MAX` の `HandshakeTimerFired` を受信した際に `!=` 判定で正しく破棄されること）。`handle_remote_event_discards_wrapped_stale_handshake_timer_generation` で検証済み。
- [x] 4.4 `RemoteEvent::OutboundEnqueued { authority, envelope, now_ms }` 処理を実装する。順序は **(a) 該当 association を取得 → (b) enqueue 前の `total_outbound_len()` を `prev` として保存 → (c) `Association::enqueue(*envelope, now_ms)`（instrument 引数なし）→ (d) enqueue 後の `total_outbound_len()` を `curr` として取得し、`prev <= high && curr > high` なら `Association::apply_backpressure(BackpressureSignal::Apply, instrument)` をエッジで発火 → (e) outbound drain helper を起動** とする。drain helper では `next_outbound` の戻り値経路で `on_send` 発火、各 dequeue 後に `total_outbound_len()` を確認し、`prev_in_drain >= low && curr_in_drain < low && state == Apply` の条件を満たした時のみ `apply_backpressure(Release, instrument)` をエッジで発火する。`enqueue` 自体には instrument 引数を渡さない。
- [x] 4.5 `RemoteEvent::ConnectionLost { authority, cause, now_ms }` 処理を実装する（再接続判断と `Association::recover(..., now_ms)` 呼び出し）。
- [x] 4.6 `RemoteEvent::TransportShutdown` 受信時は lifecycle が未停止なら `lifecycle.transition_to_shutdown_requested()` で状態を変更し、既に停止要求済みまたは停止済みなら no-op `Ok(())` とする（`shutdown_and_join` が先に `RemoteShared::shutdown()` してから wake として `TransportShutdown` を送るため冪等に扱う）。戻り値で停止判定はせず、`Remote::run` / `RemoteShared::run` 側が次の `is_terminated()` Query で停止を観測する。
- [x] 4.7 必要に応じて `RemotingLifecycleState` に `transition_to_shutdown_requested()` Command と `is_shutdown_requested(&self) -> bool` Query を追加する（既存の `transition_to_shutdown` / `is_terminated` で十分なら不要）。`Remote::is_terminated()` がこれらを観測して `true` を返せるようにする。
- [x] 4.8 outbound 駆動 helper（`Association::next_outbound` → `RemoteTransport::send(OutboundEnvelope)`）を実装する。core 側で `Codec<OutboundEnvelope>` / `Codec<InboundEnvelope>` を新設して raw bytes を transport に渡す形にはしない。
- [x] 4.9 `AssociationEffect::StartHandshake { authority, timeout, generation }` 実行経路を **2 ステップ** で実装する。
  - ステップ 1: `HandshakePdu::Req(HandshakeReq::new(local, remote))` を構築 → `RemoteTransport::send_handshake`
  - ステップ 2: `RemoteTransport::schedule_handshake_timeout(&authority, timeout, generation)`
  - ステップ 1 が `Err` の場合、ステップ 2 は呼ばない（`?` で早期 return）
- [x] 4.10 watermark 連動 backpressure 発火（`total_outbound_len` を high / low と比較し `apply_backpressure(Apply | Release)` を呼ぶ）を outbound helper に組み込む。
- [x] 4.11 復帰不能エラー時に `Err(RemotingError::TransportUnavailable)` を返す経路を実装する（`?` 伝播、`let _` 握りつぶし禁止）。
- [x] 4.12 `Remote::handle_remote_event` の unit test を追加する（fake `RemoteTransport` を持つ `Remote` で event を 1 件ずつ渡し、期待状態遷移を検証）。
- [x] 4.13 `modules/remote-core/src/core/extension/remote_shared.rs` を新設し、`pub struct RemoteShared { inner: SharedLock<Remote> }` を `#[derive(Clone)]` で定義する。`pub fn new(remote: Remote) -> Self`（内部で `SharedLock::new_with_driver::<DefaultMutex<_>>(remote)`）と `pub(crate) fn with_write` / `pub(crate) fn with_read` を実装する（公開しない）。
- [x] 4.13.1 `modules/remote-core/src/core/extension/remote_run_future.rs` を新設し、`pub struct RemoteRunFuture<'a, S: RemoteEventReceiver + ?Sized>` を定義する。`impl Future<Output = Result<(), RemotingError>> for RemoteRunFuture<'_, S>` を実装し、`Remote::run` はこの concrete type を返す。`Send` 境界は付けない。
- [x] 4.13.2 `modules/remote-core/src/core/extension/remote_shared_run_future.rs` を新設し、`pub struct RemoteSharedRunFuture<'a, S: RemoteEventReceiver + ?Sized>` を定義する。`impl Future<Output = Result<(), RemotingError>> for RemoteSharedRunFuture<'_, S>` を実装し、`RemoteShared::run` はこの concrete type を返す。`Send` 境界は付けない。
- [x] 4.14 `impl RemoteShared` に `pub fn run<'a, S: RemoteEventReceiver + ?Sized>(&'a self, receiver: &'a mut S) -> RemoteSharedRunFuture<'a, S>` を実装する（`Remote::run` の共有 wrapper、**CQS 分離**）。poll 処理：
  - `receiver.poll_recv(cx)` で event を取得（`Poll::Ready(None)` なら `Err(RemotingError::EventReceiverClosed)`）
  - `Poll::Pending` の間は lock を取らず `Poll::Pending` を返す
  - `Poll::Ready(Some(event))` で `self.with_write(|remote| remote.handle_remote_event(event))?` を実行（状態変更のみ、戻り値 `()`）
  - `if self.with_read(|remote| remote.is_terminated()) { return Poll::Ready(Ok(())); }` で Query 確認（停止判定）
  - `SharedLock` の write guard を保持したまま `Remote::run` が返す `RemoteRunFuture` を保持しない
  - `async fn` / `.await` / `impl Future` 戻り値 / `Send` 境界を作らない
  - 戻り値 bool で停止判定する経路（`Result<bool, _>` 等）を作らない
  - `RemoteSharedRunFuture` 自身が `match event`、`Association` 直接操作、`RemoteTransport` 直接呼び出しを行わないことを確認する（固有 core logic を持たせない）
- [x] 4.15 `Remote::run` と `RemoteShared::run` の unit test を追加する（fake `RemoteTransport`、in-memory `RemoteEventReceiver` で event 列を流して期待状態遷移を検証。`RemoteShared` は複数 clone から並行に `Remoting` メソッドを呼んでも進行することも確認）。
- [x] 4.16 `modules/remote-core/src/core/extension.rs` から `RemoteShared` / `RemoteRunFuture` / `RemoteSharedRunFuture` を `pub use` 経由で公開する。

## 4.5. Remoting trait のシグネチャ変更と impl 移管

- [x] 4.5.1 `modules/remote-core/src/core/extension/remoting.rs` の `Remoting` trait を `&self` ベースへ変更する。
  - `fn start(&self) -> Result<(), RemotingError>`
  - `fn shutdown(&self) -> Result<(), RemotingError>`
  - `fn quarantine(&self, address: &Address, uid: Option<u64>, reason: QuarantineReason) -> Result<(), RemotingError>`
  - `fn addresses(&self) -> Vec<Address>`（owned 戻り値、`&[Address]` から変更）
  - rustdoc を更新し「並行性の吸収責任は実装側が持つ」「すべて同期 method」を明記する
- [x] 4.5.2 `impl Remoting for Remote` を **削除** する（`Remote` は CQS 純粋ロジック層であり port を実装しない）。`Remote::start` / `shutdown` / `quarantine` / `addresses` は inherent method として残す（`RemoteShared` がデリゲートで使う）。
- [x] 4.5.3 `impl Remoting for RemoteShared` を `remote_shared.rs` に追加する。**すべて純デリゲートのみ**（`RemoteShared` は薄いラッパー、`Remote` が知らない責務を追加しない）。
  - `start(&self)`: `self.with_write(|remote| remote.start())`
  - `shutdown(&self)`: `self.with_write(|remote| remote.shutdown())` のみ（**wake しない、`event_sender` を持たない**、wake は adapter 側 `installer.shutdown_and_join` で行う）。既に停止要求済みまたは停止済みなら no-op `Ok(())` とする
  - `quarantine(&self, ..)`: `self.with_write(|remote| remote.quarantine(addr, uid, reason))`
  - `addresses(&self)`: `self.with_read(|remote| remote.addresses().to_vec())`
- [x] 4.5.4 `Remoting` trait の `addresses` 戻り値変更により他 module への影響を吸収する（`fraktor-cluster-adaptor-std-rs` 等が `&[Address]` を期待していたら `Vec<Address>` に追従）。
- [x] 4.5.5 `Remoting` trait の callers を grep で洗い出し、`&mut remoting` 受け取り箇所を `&remoting` に変更する。
  - `rtk grep -rn 'remoting: &mut\|&mut .*Remoting\|&mut dyn Remoting' modules/` で確認
- [x] 4.5.6 unit test を追加する：`RemoteShared::start` → `addresses` → `quarantine` → `shutdown` を同一 clone から呼んで期待動作を検証。複数 clone から `start` と `addresses` を並行に呼ぶ test も追加。

## 5. RemoteConfig に watermark 設定を追加

- [x] 5.1 `RemoteConfig` に `outbound_high_watermark: usize` と `outbound_low_watermark: usize` を追加する（既定値は high=1024, low=512）。
- [x] 5.2 `outbound_low_watermark < outbound_high_watermark` の validation を `RemoteConfig::validate`（または builder）で実装する。
- [x] 5.3 設定読取の unit test を追加する。

## 6. AssociationEffect::StartHandshake のセマンティクス整合

- [x] 6.1 `AssociationEffect::StartHandshake` の rustdoc を更新し、「`Remote::handle_remote_event`（`RemoteShared::run` の `with_write` 区間内）が `RemoteTransport` 経由で handshake request を送出する責務」「adapter は generation 付き timer を確保する責務」を明示する。
- [x] 6.2 既存 unit test で `recover(Some(endpoint), now)` および `associate(...)` が拡張後の `StartHandshake { authority, timeout, generation }` を返すことを確認する（既存仕様維持 + generation 追加）。

## 7. adapter 側で I/O ワーカー化と RemoteShared::run spawn 経路を追加

- [x] 7.1 `modules/remote-adaptor-std/src/std/inbound_dispatch.rs` を I/O ワーカーに変更し、TCP frame 受信後に `RemoteEvent::InboundFrameReceived` を adapter 内部 sender 経由で push するだけの処理にする。`Association::handshake_accepted` 等の直接呼び出しを削除する。
- [x] 7.2 `modules/remote-adaptor-std/src/std/tokio_remote_event_receiver.rs` を新設し、`TokioMpscRemoteEventReceiver: RemoteEventReceiver` を実装する（`tokio::sync::mpsc::Receiver<RemoteEvent>` を保持し、`poll_recv(cx)` で `Receiver::poll_recv` へ委譲。bounded、capacity は `RemoteConfig` 経由）。
- [x] 7.3 adapter 内部で `tokio::sync::mpsc::channel::<RemoteEvent>(capacity)` を生成し、`Sender` を I/O ワーカー / handshake timer task 群が clone して共有する経路を整備する（`RemoteEventSink` trait は core に追加しない）。
- [x] 7.4 `RemotingExtensionInstaller` の field を二層構造（Y 方針）+ `ExtensionInstaller::install(&self)` 契約に合わせて内部可変性で再構成する。
  - `transport: std::sync::Mutex<Option<TcpRemoteTransport>>` (既存と同じ)
  - `config: RemoteConfig` (構築後 immutable)
  - `remote_shared: std::sync::OnceLock<RemoteShared>` (install で 1 回だけ set、`OnceLock<SharedLock<Remote>>` から型変更)
  - `event_sender: std::sync::OnceLock<tokio::sync::mpsc::Sender<RemoteEvent>>` (install で set、**adapter 側で保持、`RemoteShared` には持たせない**)
  - `event_receiver: std::sync::Mutex<Option<TokioMpscRemoteEventReceiver>>` (install で set、spawn_run_task で take)
  - `run_handle: std::sync::Mutex<Option<JoinHandle<Result<(), RemotingError>>>>` (spawn_run_task で set、shutdown_and_join で take)
  - **削除する field**: 旧 `cached_addresses: Vec<Address>`（`RemoteShared::addresses` で source of truth から取得）
  - raw `Remote` 参照や raw `SharedLock<Remote>` field を持たないことを確認する（`OnceLock<RemoteShared>` のみ許容）
- [x] 7.4.1 `RemotingExtensionInstaller::install(&self, system: &ActorSystem)` を更新する（**install / start / spawn の3段階分離 + &self 契約**）。
  - `let transport = { let mut slot = self.transport.lock().map_err(|_| poisoned_err())?; slot.take().ok_or_else(|| already_installed_err())? };`
  - `Remote::with_instrument(transport, self.config.clone(), event_publisher, ...)` で `Remote` を構築
  - `RemoteShared::new(remote)` で `RemoteShared` を構築
  - `let (sender, receiver) = tokio::sync::mpsc::channel(capacity);` で channel を作成
  - `self.remote_shared.set(remote_shared).map_err(|_| already_installed_err())?;`
  - `self.event_sender.set(sender).map_err(|_| already_installed_err())?;`
  - `let mut recv_slot = self.event_receiver.lock().map_err(|_| poisoned_err())?; *recv_slot = Some(TokioMpscRemoteEventReceiver::new(receiver));`
  - **install 内では `Remote::start` を呼ばない、`tokio::spawn` もしない**（外部から start / spawn_run_task を順次呼ぶ）
- [x] 7.4.2 `installer.remote()` の戻り値型を `SharedLock<Remote>` から `RemoteShared` に変更する（`&self` 契約、内部 `OnceLock` 経由）。
  - `pub fn remote(&self) -> Result<RemoteShared, RemotingError> { self.remote_shared.get().cloned().ok_or(RemotingError::NotStarted) }` 相当
  - 既存の `SharedLock<Remote>` を返すコードと callers を破壊的変更で更新し、後方互換 shim は置かない
- [x] 7.4.3 `RemotingExtensionInstaller::spawn_run_task(&self) -> Result<(), RemotingError>` を新設する（**`&self` 契約、内部 Mutex 経由**）。
  - `let receiver = { let mut slot = self.event_receiver.lock().map_err(|_| RemotingError::TransportUnavailable)?; slot.take().ok_or(RemotingError::AlreadyRunning)? };`
  - `let run_target = self.remote_shared.get().cloned().ok_or(RemotingError::NotStarted)?;`
  - `let handle = tokio::spawn(async move { let mut receiver = receiver; run_target.run(&mut receiver).await });`
  - `let mut handle_slot = self.run_handle.lock().map_err(|_| RemotingError::TransportUnavailable)?; *handle_slot = Some(handle);`
- [x] 7.4.4 既存 test（`extension_installer/tests.rs`）を Y 方針に書き換える。
  - `installer.install(harness.system())?;` の後 `let remote = installer.remote()?;` で `RemoteShared` clone を取得
  - `remote.start()?;` で `Remoting::start` を呼ぶ（`with_lock(|r| r.start())` のような raw lock 直接呼び出しを除去）
  - 必要に応じて `installer.spawn_run_task()?;` で run task 起動
  - 停止時は `installer.shutdown_and_join().await?;` を呼ぶ（`&self` 契約のため `installer` は consume されない、テスト用の async runtime 必要）
  - `with_lock` 等の `SharedLock` 直接 API を test からも除去する
- [x] 7.4.5 PR 分割上、`RemoteShared::run` spawn 経路を有効化する前に 4.3 / 4.3.1（HandshakeTimerFired handler 実装）を同一 PR で完了させる。`StartHandshake` 経由で予約された timeout が `RemoteEvent::HandshakeTimerFired` を push した際に `Err(RemotingError::UnimplementedEvent)` で run loop を落とさないことを確認する。
- [x] 7.5 `RemotingExtensionInstaller::shutdown_and_join(&self) -> impl Future<Output = Result<(), RemotingError>>` を新設する（**`&self` 契約、握りつぶし禁止に従う、wake + 完了観測を集約**）。
  - 1. `let remote_shared = self.remote_shared.get().ok_or(RemotingError::NotStarted)?;` で `RemoteShared` 参照取得
  - 2. `remote_shared.shutdown()?;` で lifecycle terminated 遷移（既に停止要求済みまたは停止済みなら `RemoteShared::shutdown` 側が no-op `Ok(())` とする。`NotStarted` は `remote_shared` 未取得時のみ error として扱う）
  - 3. `if let Some(sender) = self.event_sender.get() { if let Err(send_err) = sender.try_send(RemoteEvent::TransportShutdown) { tracing::debug!(?send_err, "shutdown wake failed (best-effort)"); } }` で wake（**`let _ =` で握りつぶさない、log 記録**。`TransportShutdown` handler は既に停止要求済み/停止済みなら no-op）
  - 4. `let handle = { let mut slot = self.run_handle.lock().map_err(|_| RemotingError::TransportUnavailable)?; slot.take() };`
  - 5. `let Some(handle) = handle else { return Ok(()); };` で run task が無い場合は即 Ok
  - 6. `match handle.await { Ok(Ok(())) => Ok(()), Ok(Err(e)) => Err(e), Err(join_err) => { tracing::error!(?join_err, "run task join failed"); Err(RemotingError::TransportUnavailable) }, }` で完了観測 + 結果伝播
  - `RemoteShared::shutdown` 側に wake を持ち込まない（`RemoteShared` は `event_sender` を持たない、薄いラッパー原則）
- [x] 7.5.1 `Remoting::shutdown` の単独呼び出し（`shutdown_and_join` を経由しない）の挙動を test で確認する。
  - lifecycle が terminated に遷移する
  - run task は次の event 受信時に `Remote::handle_remote_event` 末尾で lifecycle terminated を観測してループ終了する
  - `poll_recv(cx)` が `Poll::Pending` のまま event が来なければ即座には停止しないことも明示的に検証する（doc test or 注釈付き unit test）
- [x] 7.6 `modules/remote-adaptor-std/src/std/effect_application.rs` から `AssociationEffect::StartHandshake` の dispatch 分岐を削除する。
- [x] 7.7 `RemoteTransport::schedule_handshake_timeout` の adapter 実装を追加する（`tokio::spawn(async move { tokio::time::sleep(timeout).await; let now_ms = monotonic_millis(); sender.send(RemoteEvent::HandshakeTimerFired { authority, generation, now_ms }).await; })` 相当）。spawn 成功で `Ok(())` を返し、内部 sleep を await しない。send 失敗は task 内で log し、`let _ =` で握りつぶさない。`Timer` trait は core に新設しない。
- [x] 7.8 adapter 側 RemoteActorRef 等の outbound 経路を `RemoteEvent::OutboundEnqueued` push に切り替える。
  - local actor の tell から到達したとき、adapter は `OutboundEnvelope` と `now_ms` を構築し、同期 `ActorRefSender::send` 内では `event_sender.try_send(RemoteEvent::OutboundEnqueued { authority, envelope: Box::new(envelope), now_ms })` で push する（`send(...).await` は使わない）
  - `try_send` の `Full` / `Closed` は `SendError` 等の caller が観測できる error に変換し、`let _` / `.ok()` で握りつぶさない
  - `AssociationRegistry` を adapter から直接 mutate しない（`enqueue` / `next_outbound` 等の呼び出しは `RemoteShared::run` 経由のみ。`Remote::handle_remote_event` の `with_write` 区間内で行われる）
  - `Result` を `?` または `match` で扱う（`let _` 禁止）
- [x] 7.8.1 actor-core provider 経由の remote sender 契約を remote-adaptor 側で固定する。
  - `StdRemoteActorRefProvider` 相当が remote authority の `ActorPath` を remote path として解決し、`RemoteActorRefSender` 相当を持つ `ActorRef` を返す
  - その `ActorRef::tell` / `ActorRefSender::send` 経路が `RemoteEvent::OutboundEnqueued` を adapter 内部 sender に push する
  - cluster-* からはこの provider surface が利用点になるため、remote-adaptor 側 test で `ActorSystem::resolve_actor_ref` 相当または provider handle 経由の解決から enqueue 到達までを確認する
  - cluster 固有の `ClusterApi::get` / `GrainRef` / topology event integration は本 task に含めず、追加 change へ分離する

## 8. adapter 側で旧 task を削除

- [x] 8.1 `modules/remote-adaptor-std/src/std/outbound_loop.rs` を削除し、`mod` 宣言と `pub(crate) use` 経路を整理する。
- [x] 8.2 `modules/remote-adaptor-std/src/std/handshake_driver.rs` を削除し、`mod` 宣言と関連 import を整理する（handshake timer 責務は task 7.7 に統合される）。
- [x] 8.3 旧 task に依存していた helper（`reconnect_backoff_policy`、`restart_counter` 等）の所属を見直し、`Remote::handle_remote_event` で使うものは `modules/remote-core/src/core/extension/` または既存の core 側既存ファイルに移動する（新規 `core/driver/` ディレクトリは作らない）。
- [x] 8.4 削除後に残る dead code（unused import、unused fields）を整理する。

## 9. 純増・variant 制約・二層構造検証

- [x] 9.1 新規追加された core 側公開型・公開 trait の数が **5 つ**（`RemoteEvent` enum + `RemoteEventReceiver` trait + `RemoteShared` 型 + `RemoteRunFuture` 型 + `RemoteSharedRunFuture` 型）であることを確認する。`RemoteTransport::schedule_handshake_timeout` は既存 trait への method 追加のため、新規 trait カウント外。
- [x] 9.2 公開禁止型・禁止 trait が core 側に追加されていないことを以下のクエリで確認する（出力が空であること）。
  - `grep -rn 'pub struct RemoteDriver\|pub trait Timer\b\|pub trait RemoteEventSink\|pub struct HandshakeGeneration\|pub struct TimerToken\|pub struct RemoteDriverHandle\|pub enum RemoteDriverOutcome' modules/remote-core/src/`
  - `grep -rn 'pub struct NoopInstrument' modules/remote-core/src/`（`NoopInstrument` は `pub(crate)` のみ許可）
  - `grep -rn 'impl<.*> RemoteInstrument for (' modules/remote-core/src/`（tuple composite 禁止）
  - `grep -rn 'impl RemoteInstrument for ()' modules/remote-core/src/`（`()` impl 禁止）
  - `grep -rn 'pub struct Remote<' modules/remote-core/src/core/extension/`（`Remote` ジェネリクス化禁止）
- [x] 9.3 `RemoteEvent` の variant 構成を確認する。
  - `grep -nE '(InboundFrameReceived|HandshakeTimerFired|OutboundEnqueued|ConnectionLost|TransportShutdown)' modules/remote-core/src/core/extension/remote_event.rs` で 5 variant が宣言されている
  - time-sensitive variant（`InboundFrameReceived` / `HandshakeTimerFired` / `OutboundEnqueued` / `ConnectionLost`）が `now_ms` を持つ
  - `OutboundEnqueued` の envelope が `Box<OutboundEnvelope>` である
  - `grep -nE '(OutboundFrameAcked|QuarantineTimerFired|BackpressureCleared)' modules/remote-core/src/core/extension/remote_event.rs` の出力が空（本 change のスコープ外）
- [x] 9.4 `RemoteTransport::schedule_handshake_timeout` 以外の scheduling 系 method が `RemoteTransport` に追加されていないことを確認する。
  - `grep -nE 'fn schedule_' modules/remote-core/src/core/transport/remote_transport.rs` で `schedule_handshake_timeout` 1 件のみ
- [x] 9.5 二層構造の遵守を以下のクエリで確認する。
  - `rtk grep -n 'impl Remoting for Remote\b' modules/remote-core/src/` の出力が空（`impl Remoting for Remote` は削除されている、`impl Remoting for RemoteShared` のみ存在）
  - `rtk grep -n 'pub fn run' modules/remote-core/src/core/extension/remote.rs` で `Remote::run(&mut self, ..) -> RemoteRunFuture` が定義されている（排他所有時の core event loop）
  - `rtk grep -nE 'pub async fn run|-> impl .*Future|run\\(self' modules/remote-core/src/core/extension/remote.rs` の出力が空（async fn / impl Future 戻り値 / consume `self` 形は禁止）
  - `rtk grep -n 'pub fn run' modules/remote-core/src/core/extension/remote_shared.rs` で `RemoteShared::run(&self, ..) -> RemoteSharedRunFuture` が定義されている
  - `rtk grep -nE 'pub async fn run|-> impl .*Future' modules/remote-core/src/core/extension/remote_shared.rs` の出力が空
  - `rtk grep -nE 'pub fn .*&mut self\|pub async fn .*&mut self' modules/remote-core/src/core/extension/remote_shared.rs` の出力が空（`RemoteShared` の公開 method はすべて `&self`）
  - `rtk grep -nE 'RemoteEventReceiver: Send|fn recv\\(|async fn recv|impl Future<Output = Option<RemoteEvent>>' modules/remote-core/src/core/extension/remote_event_receiver.rs` の出力が空
  - `rtk grep -nE 'pub fn handle_remote_event.*Result<bool' modules/remote-core/src/` の出力が空（CQS 違反、戻り値で停止判定する形を排除）
  - `rtk grep -nE 'pub fn is_terminated\(&self\)' modules/remote-core/src/core/extension/remote.rs` で 1 件ヒット（CQS Query 確認）
- [x] 9.5.1 `RemoteShared` の薄いラッパー原則の遵守を以下のクエリで確認する。
  - `rtk grep -n 'event_sender\|EventSink\|tokio' modules/remote-core/src/core/extension/remote_shared.rs` の出力が空（`RemoteShared` は `Remote` が知らない responsibility を持たない）
  - `rtk grep -n 'match event\|RemoteTransport\|Association' modules/remote-core/src/core/extension/remote_shared.rs` の出力が空（`RemoteShared` に固有 core logic を持たせない）
  - `rtk grep -n 'RemoteRunFuture\|remote.run' modules/remote-core/src/core/extension/remote_shared_run_future.rs` の出力が空（write guard から `Remote::run` future を保持しない）
  - `RemoteShared` の field が `inner: SharedLock<Remote>` 1 個のみであることを目視確認
  - `cargo tree -p fraktor-remote-core-rs` で `tokio` 等の runtime crate への依存が含まれていないことを確認
- [x] 9.6 adapter 側 installer に raw `Remote` 参照がないことを確認する。
  - `rtk grep -nE 'Arc<.*Remote\b|Mutex<.*Remote\b|RwLock<.*Remote\b|SharedLock<.*Remote\b' modules/remote-adaptor-std/src/` を実行し、`OnceLock<RemoteShared>` field 以外に raw 参照が残っていないことを確認する
  - `rtk grep -n 'cached_addresses' modules/remote-adaptor-std/src/` の出力が空（addresses cache は削除されている、`RemoteShared::addresses` で取得）
  - `installer.remote()` の戻り値型が `Result<RemoteShared, _>` であることを確認（`SharedLock<Remote>` は返さない）
- [x] 9.6.1 adapter installer の wake / 完了観測責務の遵守を確認する（`&self` 契約 + 内部可変性）。
  - `RemotingExtensionInstaller` に `event_sender: std::sync::OnceLock<tokio::sync::mpsc::Sender<RemoteEvent>>` field が存在
  - `RemotingExtensionInstaller` に `event_receiver: std::sync::Mutex<Option<TokioMpscRemoteEventReceiver>>` field が存在
  - `RemotingExtensionInstaller` に `run_handle: std::sync::Mutex<Option<JoinHandle<Result<(), RemotingError>>>>` field が存在
  - `RemotingExtensionInstaller::shutdown_and_join(&self) -> impl Future<Output = Result<(), RemotingError>>` 等の adapter 固有 async API が存在（**`&self`、`self` consume ではない**）
  - `RemotingExtensionInstaller::spawn_run_task(&self) -> Result<(), RemotingError>` 等の明示的 spawn API が存在（**`&self`、`&mut self` ではない**）
  - `RemotingExtensionInstaller::install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError>` の `ExtensionInstaller` trait 実装が `&self` 契約に従っている
- [x] 9.6.2 shutdown_and_join 内の握りつぶし禁止を確認する。
  - `rtk grep -nE 'let _ = self\.remote_shared|let _ = self\.event_sender' modules/remote-adaptor-std/src/` の出力が空
  - `shutdown_and_join` の実装ソースに `match` または `if let Err` が存在し、各 `Result` を明示的に扱っている
  - `tracing::debug!` / `tracing::error!` 等で error 値を log に記録している経路が確認できる
- [x] 9.7 net file delta を確認する。実差分は旧 adapter association 駆動 helper（registry / shared / effect_application / inbound quarantine / outbound loop / handshake driver / peer match / reconnect / system delivery / 旧 tests）を削除したため当初想定の `+4` 程度ではなく純減寄り。core 新規 run/shared future 群と adapter `tokio_remote_event_receiver` の追加、旧直接駆動層の残存なしを確認済み。

## 10. テスト

- [x] 10.1 `rtk cargo test -p fraktor-remote-core-rs` を実行し、green を確認する（二層構造への変更後、`Remote` / `RemoteShared` / `Remoting` trait の新シグネチャで test が green になること）。
- [x] 10.2 `rtk cargo test -p fraktor-remote-adaptor-std-rs` を実行し、green を確認する（installer の `RemoteShared` 化後、test が green になること）。
- [x] 10.2.1 remote-adaptor 側に provider 経由 enqueue の test を追加または更新する。
  - `StdRemoteActorRefProvider` / provider handle 経由で remote path を解決できること
  - 解決した `ActorRef` への `tell` が `RemoteEvent::OutboundEnqueued` を adapter 内部 receiver から観測できること
  - `try_send` 失敗時に caller が `SendError` 等を観測できること
- [x] 10.3 `rtk cargo test -p fraktor-cluster-adaptor-std-rs` を実行し、依存先の green を確認する（`Remoting` trait シグネチャ変更の波及を吸収するだけで、cluster/remoting end-to-end 利用性の証明は追加 change へ分離）。
- [x] 10.4 handshake / quarantine / watermark backpressure / instrument 通知 / handshake generation 破棄の integration test を追加または更新する。
  - `installer.install` → `installer.remote().start()` → `installer.spawn_run_task()` の起動順序を検証
  - `Remoting::quarantine` を run と並行して呼ぶケースも含めて検証（per-event lock の隙間で進行することを確認）
  - `installer.shutdown_and_join().await` で graceful shutdown が成立することを検証
  - `Remoting::shutdown` 単独呼び出し後、event 1 件で run loop が終了することも検証
- [x] 10.5 showcase（`showcases/std/remote_lifecycle/` 等）が新 API で起動することを確認する。
- [x] 10.6 follow-up change の境界を確認する。
  - 本 change は remote 側契約（provider 経由 enqueue まで）で完了とする
  - `ClusterApi::get` / `GrainRef` から remote delivery までの integration test、remoting lifecycle event の cluster topology 反映、`subscribe_remoting_events` の購読 lifetime 修正は追加 change（候補名: `prove-cluster-uses-remote-adaptor`）で扱う

## 11. 検証

- [x] 11.1 dylint（mod-file、module-wiring、type-per-file、tests-location、use-placement、rustdoc、cfg-std-forbid、ambiguous-suffix）を `rtk cargo clippy` 系で確認する。
- [x] 11.2 `rtk ./scripts/ci-check.sh ai all` を最後まで完了させる。
