## 1. instrument 配線基盤を core 側に整える（dyn dispatch 採用、純増ゼロ方針）

- [x] 1.1 `modules/remote-core/src/core/instrument/noop_instrument.rs` を新設し、`pub(crate) struct NoopInstrument;` と `impl RemoteInstrument for NoopInstrument` を追加する（1 file 1 type ルールに従い専用ファイルを作る、外部公開しない）。すべての method 本体は空。**`pub` での公開は禁止**（`pub(crate)` 限定）。
- [x] 1.2 `modules/remote-core/src/core/extension/remote.rs` の `Remote` に `instrument: alloc::boxed::Box<dyn RemoteInstrument + Send>` フィールドを追加する。**`Remote` に型パラメータ `<I>` を導入してはならない**。
- [x] 1.3 `Remote::new(transport, config, event_publisher)` を更新し、内部で `instrument: Box::new(NoopInstrument)` を割り当てる。既存呼出シグネチャは変更なし（フィールド追加のみのため非破壊）。
- [x] 1.4 `Remote::with_instrument(transport, config, event_publisher, instrument: Box<dyn RemoteInstrument + Send>) -> Self` を新規 public API として追加する。
- [x] 1.5 `Remote::set_instrument(&mut self, instrument: Box<dyn RemoteInstrument + Send>)` を新規 public API として追加し、rustdoc に「`run` 進行中に呼ばないこと」を明示する。
- [x] 1.6 `modules/remote-core/src/core/instrument/flight_recorder.rs` に `impl RemoteInstrument for RemotingFlightRecorder` を追加し、record 系メソッドを RemoteInstrument hook 経由でも発火可能にする。
- [ ] 1.7 instrument 単体 unit test を追加する。`Remote::new` 既定（NoopInstrument）と `Remote::with_instrument(.., Box::new(RemotingFlightRecorder::new(..)))` の両構築を確認し、event loop で hook が ring buffer に届くことを検証する。
- [x] 1.8 **tuple composite と `() impl` は追加しない** ことを確認する。`grep -n 'impl<.*> RemoteInstrument for (' modules/remote-core/src/` および `grep -n 'impl RemoteInstrument for ()' modules/remote-core/src/` の出力が空であること。

## 2. Association に instrument hook と watermark 用 query を追加（newtype を作らない）

- [ ] 2.1 `Association::associate` のシグネチャに `instrument: &mut dyn RemoteInstrument` を追加し、`record_handshake(authority, HandshakePhase::Started, now_ms)` を内部から呼び出す。**型パラメータ `<I>` は導入しない**。
- [ ] 2.2 `Association::handshake_accepted` に同様の `instrument: &mut dyn RemoteInstrument` 引数を追加し、`record_handshake(.., HandshakePhase::Accepted, ..)` を呼び出す。
- [ ] 2.3 `Association::handshake_timed_out` に `instrument: &mut dyn RemoteInstrument` 引数を追加し、`record_handshake(.., HandshakePhase::Rejected, ..)` を呼び出す。
- [ ] 2.4 `Association::quarantine` に `instrument: &mut dyn RemoteInstrument` 引数を追加し、`record_quarantine(authority, reason, now_ms)` を呼び出す。
- [ ] 2.5 `Association::apply_backpressure` に `instrument: &mut dyn RemoteInstrument` 引数を追加し、`record_backpressure(authority, signal, correlation_id, now_ms)` を呼び出す（既存 `BackpressureSignal::Apply` / `Release` をそのまま流用、新 variant 追加なし）。
- [x] 2.6 `Association::next_outbound` の戻り値経路（または直近の `Remote::handle_remote_event` 呼び出し点）で `on_send(envelope)` を発火する経路を確立する。
- [x] 2.7 inbound dispatch 経路で `on_receive(envelope)` を発火するための公開 method を Association に追加する（または既存 method に instrument 引数を渡す）。
- [x] 2.8 `Association::total_outbound_len(&self) -> usize` を追加する（`SendQueue` の system + user 合計、deferred は含めない）。
- [x] 2.9 `Association` に `handshake_generation: u64` フィールドを追加し、`Handshaking` 状態に入るたびに `wrapping_add(1)` で +1 する（**`HandshakeGeneration` newtype は新設しない**）。
- [x] 2.10 `AssociationEffect::StartHandshake` を `{ authority: TransportEndpoint, timeout: core::time::Duration, generation: u64 }` に拡張し、rustdoc に「`Remote::handle_remote_event`（`RemoteShared::run` の `with_write` 区間内）が実行する責務」と「adapter は generation 付き timer を確保する責務」を明示する。
- [x] 2.11 上記変更の unit test を追加する（各 hook 呼び出し点で記録された FlightRecorder snapshot で順序を検証）。
- [ ] 2.12 既存の `Association::*_with_instrument` 併設 API（例: `associate_with_instrument` / `accept_handshake_request_with_instrument` / `accept_handshake_response_with_instrument` 等）を削除し、instrument 必須シグネチャ 1 本に統合する（`remote-core-association-state-machine` capability の「最終形では `*_with_instrument` 併設 API を残さない」要件に従う）。
  - `rtk grep -rn '_with_instrument' modules/remote-core/src/ modules/remote-adaptor-std/src/` で対象を洗い出す
  - 各 callers を instrument 必須シグネチャに更新
  - `rtk grep -rn '_with_instrument' modules/` の出力が空（または `pub(crate)` 内部 helper のみ）になることを確認

## 3. core 側に RemoteEvent / RemoteEventReceiver / RemoteTransport::schedule_handshake_timeout を追加

- [x] 3.1 `modules/remote-core/src/core/extension/remote_event.rs` を新設し、`pub enum RemoteEvent` を closed enum で定義する（**5 variant のみ**: `InboundFrameReceived { authority, frame: Vec<u8> }` / `HandshakeTimerFired { authority, generation: u64 }` / `OutboundEnqueued { authority, envelope: OutboundEnvelope }` / `ConnectionLost { authority, cause: ConnectionLostCause }` / `TransportShutdown`）。**`OutboundFrameAcked` / `QuarantineTimerFired` / `BackpressureCleared` は本 change では追加しない**（必要時に別 change で variant 追加 + scheduling 経路 MODIFIED を一緒に行う）。
- [x] 3.2 `modules/remote-core/src/core/extension/remote_event_receiver.rs` を新設し、`pub trait RemoteEventReceiver: Send` と `fn recv(&mut self) -> impl Future<Output = Option<RemoteEvent>> + Send + '_` を定義する。
- [x] 3.3 `modules/remote-core/src/core/transport/remote_transport.rs` の `RemoteTransport` trait に `fn schedule_handshake_timeout(&mut self, authority: &TransportEndpoint, timeout: core::time::Duration, generation: u64) -> Result<(), TransportError>` を追加する（同期 method、`async fn` は使わない）。rustdoc に「adapter は満了時に `RemoteEvent::HandshakeTimerFired { authority, generation }` を adapter 内部 sender 経由で receiver に push する責務を持つ」を明記する。
- [x] 3.4 `modules/remote-core/src/core/extension.rs`（または `mod.rs`）から `RemoteEvent` / `RemoteEventReceiver` を `pub use` 経由で公開する。
- [x] 3.5 **`RemoteEventSink` / `Timer` / `RemoteDriver*` 系の trait・型は新設しないことを確認** する（dylint module-wiring と code review で担保）。

## 4. Remote::handle_remote_event と RemoteShared::run を実装

- [ ] 4.0 **既存の `Remote::run(self, ..)` を一旦削除**し、本 change の二層構造に合わせて `Remote::handle_remote_event` + `RemoteShared::run` に分解する。実装中の途中状態を許容しないため、削除と新規追加を同一 PR で行う。
- [ ] 4.1 `modules/remote-core/src/core/extension/remote.rs` に `impl Remote` で `pub fn handle_remote_event(&mut self, event: RemoteEvent) -> Result<bool, RemotingError>` の skeleton を追加する。戻り値が `true` ならループ終了（`TransportShutdown` 受信または lifecycle terminated 観測時）。`Remote` 自体には型パラメータ `<I>` を持たせない（instrument は `Box<dyn RemoteInstrument + Send>` フィールド経由）。event 処理で instrument と他 field を同時に扱う場合は、field 単位の split borrow が成立する helper へ切り出す。
- [ ] 4.2 `RemoteEvent::InboundFrameReceived` 処理を `handle_remote_event` 内に実装する（`Codec::decode` → Association inbound dispatch → instrument `on_receive`）。
- [ ] 4.3 `RemoteEvent::HandshakeTimerFired { generation }` 処理を実装する（`Association.handshake_generation` と `!=` で比較し、不一致時は event を破棄。一致時のみ `Association::handshake_timed_out` を呼ぶ。`>` / `<` 比較は使わない — `wrapping_add` の wrap で stale 判定が漏れないようにする）。
- [ ] 4.3.1 wrap 境界の unit test を追加する（`handshake_generation = u64::MAX` → 次回 `Handshaking` で `0` になり、古い `g_event = u64::MAX` の `HandshakeTimerFired` を受信した際に `!=` 判定で正しく破棄されること）。
- [ ] 4.4 `RemoteEvent::OutboundEnqueued { authority, envelope }` 処理を実装する。順序は **(a) 該当 association を取得 → (b) enqueue 前の `total_outbound_len()` を `prev` として保存 → (c) `Association::enqueue(envelope)`（instrument 引数なし）→ (d) enqueue 後の `total_outbound_len()` を `curr` として取得し、`prev <= high && curr > high` なら `Association::apply_backpressure(BackpressureSignal::Apply, instrument)` をエッジで発火 → (e) outbound drain helper を起動** とする。drain helper では `next_outbound` の戻り値経路で `on_send` 発火、各 dequeue 後に `total_outbound_len()` を確認し、`prev_in_drain >= low && curr_in_drain < low && state == Apply` の条件を満たした時のみ `apply_backpressure(Release, instrument)` をエッジで発火する。`enqueue` 自体には instrument 引数を渡さない。
- [ ] 4.5 `RemoteEvent::ConnectionLost` 処理を実装する（再接続判断と `Association::recover` 呼び出し）。
- [ ] 4.6 `RemoteEvent::TransportShutdown` で `Ok(true)` を返す（戻り値 true で `RemoteShared::run` 側がループ終了する）。
- [ ] 4.7 lifecycle terminated 観測時にも `Ok(true)` を返す経路を `handle_remote_event` 末尾に追加する（`Remoting::shutdown` → `Remote::shutdown` で lifecycle が terminated に遷移した場合、`event_sender.try_send(TransportShutdown)` の wake が失敗しても次の event 処理で安全に終了できるようにする）。
- [ ] 4.8 outbound 駆動 helper（`Association::next_outbound` → `Codec::encode` → `RemoteTransport::send`）を実装する。
- [ ] 4.9 `AssociationEffect::StartHandshake { authority, timeout, generation }` 実行経路を **2 ステップ** で実装する。
  - ステップ 1: `HandshakePdu::Req(HandshakeReq::new(local, remote))` を構築 → `RemoteTransport::send_handshake`
  - ステップ 2: `RemoteTransport::schedule_handshake_timeout(&authority, timeout, generation)`
  - ステップ 1 が `Err` の場合、ステップ 2 は呼ばない（`?` で早期 return）
- [ ] 4.10 watermark 連動 backpressure 発火（`total_outbound_len` を high / low と比較し `apply_backpressure(Apply | Release)` を呼ぶ）を outbound helper に組み込む。
- [ ] 4.11 復帰不能エラー時に `Err(RemotingError::TransportUnavailable)` を返す経路を実装する（`?` 伝播、`let _` 握りつぶし禁止）。
- [ ] 4.12 `Remote::handle_remote_event` の unit test を追加する（fake `RemoteTransport` を持つ `Remote` で event を 1 件ずつ渡し、期待状態遷移を検証）。
- [ ] 4.13 `modules/remote-core/src/core/extension/remote_shared.rs` を新設し、`pub struct RemoteShared { inner: SharedLock<Remote> }` を `#[derive(Clone)]` で定義する。`pub fn new(remote: Remote) -> Self`（内部で `SharedLock::new_with_driver::<DefaultMutex<_>>(remote)`）と `pub(crate) fn with_write` / `pub(crate) fn with_read` を実装する（公開しない）。
- [ ] 4.14 `impl RemoteShared` に `pub async fn run<S: RemoteEventReceiver>(&self, receiver: &mut S) -> Result<(), RemotingError>` を実装する。per-event lock ループ：`receiver.recv().await` で event を取得 → `with_write(|remote| remote.handle_remote_event(event))?` → 戻り値 `true` で `Ok(())` を返す。`recv` が `None` を返したら `Err(RemotingError::EventReceiverClosed)`。`await` 越しにロックを保持しないことを実装で確認する。
- [ ] 4.15 `RemoteShared::run` の unit test を追加する（fake `RemoteTransport`、in-memory `RemoteEventReceiver` で event 列を流して期待状態遷移を検証。複数 clone から並行に `Remoting` メソッドを呼んでも進行することも確認）。
- [ ] 4.16 `modules/remote-core/src/core/extension.rs` から `RemoteShared` を `pub use` 経由で公開する。

## 4.5. Remoting trait のシグネチャ変更と impl 移管

- [ ] 4.5.1 `modules/remote-core/src/core/extension/remoting.rs` の `Remoting` trait を `&self` ベースへ変更する。
  - `fn start(&self) -> Result<(), RemotingError>`
  - `fn shutdown(&self) -> Result<(), RemotingError>`
  - `fn quarantine(&self, address: &Address, uid: Option<u64>, reason: QuarantineReason) -> Result<(), RemotingError>`
  - `fn addresses(&self) -> Vec<Address>`（owned 戻り値、`&[Address]` から変更）
  - rustdoc を更新し「並行性の吸収責任は実装側が持つ」「すべて同期 method」を明記する
- [ ] 4.5.2 `impl Remoting for Remote` を **削除** する（`Remote` は CQS 純粋ロジック層であり port を実装しない）。`Remote::start` / `shutdown` / `quarantine` / `addresses` は inherent method として残す（`RemoteShared` がデリゲートで使う）。
- [ ] 4.5.3 `impl Remoting for RemoteShared` を `remote_shared.rs` に追加する。**すべて純デリゲートのみ**（`RemoteShared` は薄いラッパー、`Remote` が知らない責務を追加しない）。
  - `start(&self)`: `self.with_write(|remote| remote.start())`
  - `shutdown(&self)`: `self.with_write(|remote| remote.shutdown())` のみ（**wake しない、`event_sender` を持たない**、wake は adapter 側 `installer.shutdown_and_join` で行う）
  - `quarantine(&self, ..)`: `self.with_write(|remote| remote.quarantine(addr, uid, reason))`
  - `addresses(&self)`: `self.with_read(|remote| remote.addresses().to_vec())`
- [ ] 4.5.4 `Remoting` trait の `addresses` 戻り値変更により他 module への影響を吸収する（`fraktor-cluster-adaptor-std-rs` 等が `&[Address]` を期待していたら `Vec<Address>` に追従）。
- [ ] 4.5.5 `Remoting` trait の callers を grep で洗い出し、`&mut remoting` 受け取り箇所を `&remoting` に変更する。
  - `rtk grep -rn 'remoting: &mut\|&mut .*Remoting\|&mut dyn Remoting' modules/` で確認
- [ ] 4.5.6 unit test を追加する：`RemoteShared::start` → `addresses` → `quarantine` → `shutdown` を同一 clone から呼んで期待動作を検証。複数 clone から `start` と `addresses` を並行に呼ぶ test も追加。

## 5. RemoteConfig に watermark 設定を追加

- [x] 5.1 `RemoteConfig` に `outbound_high_watermark: usize` と `outbound_low_watermark: usize` を追加する（既定値は high=1024, low=512）。
- [x] 5.2 `outbound_low_watermark < outbound_high_watermark` の validation を `RemoteConfig::validate`（または builder）で実装する。
- [x] 5.3 設定読取の unit test を追加する。

## 6. AssociationEffect::StartHandshake のセマンティクス整合

- [x] 6.1 `AssociationEffect::StartHandshake` の rustdoc を更新し、「`Remote::handle_remote_event`（`RemoteShared::run` の `with_write` 区間内）が `RemoteTransport` 経由で handshake request を送出する責務」「adapter は generation 付き timer を確保する責務」を明示する。
- [x] 6.2 既存 unit test で `recover(Some(endpoint), now)` および `associate(...)` が拡張後の `StartHandshake { authority, timeout, generation }` を返すことを確認する（既存仕様維持 + generation 追加）。

## 7. adapter 側で I/O ワーカー化と RemoteShared::run spawn 経路を追加

- [ ] 7.1 `modules/remote-adaptor-std/src/std/inbound_dispatch.rs` を I/O ワーカーに変更し、TCP frame 受信後に `RemoteEvent::InboundFrameReceived` を adapter 内部 sender 経由で push するだけの処理にする。`Association::handshake_accepted` 等の直接呼び出しを削除する。
- [x] 7.2 `modules/remote-adaptor-std/src/std/tokio_remote_event_receiver.rs` を新設し、`TokioMpscRemoteEventReceiver: RemoteEventReceiver` を実装する（`tokio::sync::mpsc::Receiver<RemoteEvent>` を保持。bounded、capacity は `RemoteConfig` 経由）。
- [ ] 7.3 adapter 内部で `tokio::sync::mpsc::channel::<RemoteEvent>(capacity)` を生成し、`Sender` を I/O ワーカー / handshake timer task 群が clone して共有する経路を整備する（`RemoteEventSink` trait は core に追加しない）。
- [ ] 7.4 `RemotingExtensionInstaller` の field を二層構造（Y 方針）に合わせて再構成する。
  - `remote_shared: RemoteShared` を保持する（`RemoteShared::new(remote)` で構築）
  - `event_sender: tokio::sync::mpsc::Sender<RemoteEvent>` を保持する（**adapter 側で保持、`RemoteShared` には持たせない**）
  - `event_receiver: Option<TokioMpscRemoteEventReceiver>` を保持する（spawn_run_task で take して spawn task に move）
  - `run_handle: Option<JoinHandle<Result<(), RemotingError>>>` を保持する（spawn_run_task で Some に、shutdown_and_join で take）
  - **削除する field**: 旧 `OnceLock<SharedLock<Remote>>` / `cached_addresses: Vec<Address>`（`RemoteShared::addresses` で source of truth から取得するため）
  - raw `Remote` 参照や raw `SharedLock<Remote>` field を持たないことを確認する
- [ ] 7.4.1 `RemotingExtensionInstaller::install` を更新する（**install / start / spawn の3段階分離**）。
  - `Remote::with_instrument(transport, config, event_publisher, instrument)` で `Remote` を構築
  - `RemoteShared::new(remote)` で `RemoteShared` を構築し `remote_shared` field に保存
  - `tokio::sync::mpsc::channel(capacity)` で channel を作り `event_sender` / `event_receiver` を field に保存
  - **install 内では `Remote::start` を呼ばない、`tokio::spawn` もしない**（外部から start / spawn_run_task を順次呼ぶ）
- [ ] 7.4.2 `installer.remote()` の戻り値型を `SharedLock<Remote>` から `RemoteShared` に変更する。
  - `pub fn remote(&self) -> RemoteShared { self.remote_shared.clone() }` 相当
  - 既存の `SharedLock<Remote>` を返すコードと callers を破壊的変更で更新（CLAUDE.md「後方互換は不要」）
- [ ] 7.4.3 `RemotingExtensionInstaller::spawn_run_task(&mut self) -> Result<(), RemotingError>` を新設する。
  - `let receiver = self.event_receiver.take().ok_or(RemotingError::AlreadyRunning)?;`
  - `let run_target = self.remote_shared.clone();`
  - `let handle = tokio::spawn(async move { let mut receiver = receiver; run_target.run(&mut receiver).await });`
  - `self.run_handle = Some(handle);` で保存
- [ ] 7.4.4 既存 test（`extension_installer/tests.rs`）を Y 方針に書き換える。
  - `installer.install(harness.system())?;` の後 `let remote = installer.remote();` で `RemoteShared` clone を取得
  - `remote.start()?;` で `Remoting::start` を呼ぶ（`with_lock(|r| r.start())` のような raw lock 直接呼び出しを除去）
  - 必要に応じて `installer.spawn_run_task()?;` で run task 起動
  - 停止時は `installer.shutdown_and_join().await?;` を呼ぶ（テスト用の async runtime 必要）
  - `with_lock` 等の `SharedLock` 直接 API を test からも除去する
- [ ] 7.4.5 PR 分割上、`RemoteShared::run` spawn 経路を有効化する前に 4.3 / 4.3.1（HandshakeTimerFired handler 実装）を同一 PR で完了させる。`StartHandshake` 経由で予約された timeout が `RemoteEvent::HandshakeTimerFired` を push した際に `Err(RemotingError::UnimplementedEvent)` で run loop を落とさないことを確認する。
- [ ] 7.5 `RemotingExtensionInstaller::shutdown_and_join(self) -> impl Future<Output = Result<(), RemotingError>>` を新設する（**adapter 固有 async surface、wake + 完了観測を集約**）。
  - 1. `let _ = self.remote_shared.shutdown();`（`RemoteShared::shutdown` で lifecycle terminated 遷移、戻り値の `Result` は best-effort で無視可、log 記録は望ましい）
  - 2. `let _ = self.event_sender.try_send(RemoteEvent::TransportShutdown);`（同期 try_send、`await` しない、Full / Closed 失敗は無視）
  - 3. `let Some(handle) = self.run_handle.take() else { return Ok(()); };` で `JoinHandle` を取り出し
  - 4. `match handle.await { Ok(Ok(())) => Ok(()), Ok(Err(e)) => Err(e), Err(_) => Err(RemotingError::TransportUnavailable) }` で完了観測 + 結果伝播
  - `RemoteShared::shutdown` 側に wake を持ち込まない（`RemoteShared` は `event_sender` を持たない、薄いラッパー原則）
- [ ] 7.5.1 `Remoting::shutdown` の単独呼び出し（`shutdown_and_join` を経由しない）の挙動を test で確認する。
  - lifecycle が terminated に遷移する
  - run task は次の event 受信時に `Remote::handle_remote_event` 末尾で lifecycle terminated を観測してループ終了する
  - `recv().await` で blocked のまま event が来なければ即座には停止しないことも明示的に検証する（doc test or 注釈付き unit test）
- [ ] 7.6 `modules/remote-adaptor-std/src/std/effect_application.rs` から `AssociationEffect::StartHandshake` の dispatch 分岐を削除する。
- [x] 7.7 `RemoteTransport::schedule_handshake_timeout` の adapter 実装を追加する（`tokio::spawn(async move { tokio::time::sleep(timeout).await; sender.send(RemoteEvent::HandshakeTimerFired { authority, generation }).await; })` 相当）。spawn 成功で `Ok(())` を返し、内部 sleep を await しない。`Timer` trait は core に新設しない。
- [ ] 7.8 adapter 側 RemoteActorRef 等の outbound 経路を `RemoteEvent::OutboundEnqueued` push に切り替える。
  - local actor の tell から到達したとき、adapter は `OutboundEnvelope` を構築し、`event_sender.send(RemoteEvent::OutboundEnqueued { authority, envelope }).await` で push する
  - `AssociationRegistry` を adapter から直接 mutate しない（`enqueue` / `next_outbound` 等の呼び出しは `RemoteShared::run` 経由のみ。`Remote::handle_remote_event` の `with_write` 区間内で行われる）
  - `Result` を `?` または `match` で扱う（`let _` 禁止）

## 8. adapter 側で旧 task を削除

- [ ] 8.1 `modules/remote-adaptor-std/src/std/outbound_loop.rs` を削除し、`mod` 宣言と `pub(crate) use` 経路を整理する。
- [ ] 8.2 `modules/remote-adaptor-std/src/std/handshake_driver.rs` を削除し、`mod` 宣言と関連 import を整理する（handshake timer 責務は task 7.7 に統合される）。
- [ ] 8.3 旧 task に依存していた helper（`reconnect_backoff_policy`、`restart_counter` 等）の所属を見直し、`Remote::handle_remote_event` で使うものは `modules/remote-core/src/core/extension/` または既存の core 側既存ファイルに移動する（新規 `core/driver/` ディレクトリは作らない）。
- [ ] 8.4 削除後に残る dead code（unused import、unused fields）を整理する。

## 9. 純増・variant 制約・二層構造検証

- [ ] 9.1 新規追加された core 側公開型・公開 trait の数が **3 つ**（`RemoteEvent` enum + `RemoteEventReceiver` trait + `RemoteShared` 型）であることを確認する。`RemoteTransport::schedule_handshake_timeout` は既存 trait への method 追加のため、新規 trait カウント外。
- [x] 9.2 公開禁止型・禁止 trait が core 側に追加されていないことを以下のクエリで確認する（出力が空であること）。
  - `grep -rn 'pub struct RemoteDriver\|pub trait Timer\b\|pub trait RemoteEventSink\|pub struct HandshakeGeneration\|pub struct TimerToken\|pub struct RemoteDriverHandle\|pub enum RemoteDriverOutcome' modules/remote-core/src/`
  - `grep -rn 'pub struct NoopInstrument' modules/remote-core/src/`（`NoopInstrument` は `pub(crate)` のみ許可）
  - `grep -rn 'impl<.*> RemoteInstrument for (' modules/remote-core/src/`（tuple composite 禁止）
  - `grep -rn 'impl RemoteInstrument for ()' modules/remote-core/src/`（`()` impl 禁止）
  - `grep -rn 'pub struct Remote<' modules/remote-core/src/core/extension/`（`Remote` ジェネリクス化禁止）
- [x] 9.3 `RemoteEvent` の variant 構成を確認する。
  - `grep -nE '(InboundFrameReceived|HandshakeTimerFired|OutboundEnqueued|ConnectionLost|TransportShutdown)' modules/remote-core/src/core/extension/remote_event.rs` で 5 variant が宣言されている
  - `grep -nE '(OutboundFrameAcked|QuarantineTimerFired|BackpressureCleared)' modules/remote-core/src/core/extension/remote_event.rs` の出力が空（本 change のスコープ外）
- [x] 9.4 `RemoteTransport::schedule_handshake_timeout` 以外の scheduling 系 method が `RemoteTransport` に追加されていないことを確認する。
  - `grep -nE 'fn schedule_' modules/remote-core/src/core/transport/remote_transport.rs` で `schedule_handshake_timeout` 1 件のみ
- [ ] 9.5 二層構造の遵守を以下のクエリで確認する。
  - `rtk grep -n 'impl Remoting for Remote\b' modules/remote-core/src/` の出力が空（`impl Remoting for Remote` は削除されている、`impl Remoting for RemoteShared` のみ存在）
  - `rtk grep -n 'pub async fn run\|pub fn run' modules/remote-core/src/core/extension/remote.rs` の出力が空（`Remote` 自身に `run` を持たせない）
  - `rtk grep -n 'pub async fn run' modules/remote-core/src/core/extension/remote_shared.rs` で `RemoteShared::run` が定義されている
  - `rtk grep -nE 'pub fn .*&mut self\|pub async fn .*&mut self' modules/remote-core/src/core/extension/remote_shared.rs` の出力が空（`RemoteShared` の公開 method はすべて `&self`）
- [ ] 9.5.1 `RemoteShared` の薄いラッパー原則の遵守を以下のクエリで確認する。
  - `rtk grep -n 'event_sender\|EventSink\|tokio' modules/remote-core/src/core/extension/remote_shared.rs` の出力が空（`RemoteShared` は `Remote` が知らない responsibility を持たない）
  - `RemoteShared` の field が `inner: SharedLock<Remote>` 1 個のみであることを目視確認
  - `cargo tree -p fraktor-remote-core-rs` で `tokio` 等の runtime crate への依存が含まれていないことを確認
- [ ] 9.6 adapter 側 installer に raw `Remote` 参照がないことを確認する。
  - `rtk grep -nE 'Arc<.*Remote\b|Mutex<.*Remote\b|RwLock<.*Remote\b|SharedLock<.*Remote\b' modules/remote-adaptor-std/src/` を実行し、`remote_shared: RemoteShared` field 以外に raw 参照が残っていないことを確認する
  - `rtk grep -n 'cached_addresses' modules/remote-adaptor-std/src/` の出力が空（addresses cache は削除されている、`RemoteShared::addresses` で取得）
  - `installer.remote()` の戻り値型が `RemoteShared` であることを確認（`SharedLock<Remote>` は返さない）
- [ ] 9.6.1 adapter installer の wake / 完了観測責務の遵守を確認する。
  - `RemotingExtensionInstaller` に `event_sender: tokio::sync::mpsc::Sender<RemoteEvent>` field が存在
  - `RemotingExtensionInstaller::shutdown_and_join(self) -> impl Future<Output = Result<(), RemotingError>>` 等の adapter 固有 async API が存在
  - `RemotingExtensionInstaller::spawn_run_task(&mut self) -> Result<(), RemotingError>` 等の明示的 spawn API が存在（install と spawn の分離）
- [ ] 9.7 net file delta が `+2` 程度（core 新規 `remote_event.rs` + `remote_event_receiver.rs` + `remote_shared.rs` / adapter 新規 `tokio_remote_event_receiver.rs` / adapter 削除 `outbound_loop.rs` + `handshake_driver.rs`）であることを確認する。

## 10. テスト

- [ ] 10.1 `rtk cargo test -p fraktor-remote-core-rs` を実行し、green を確認する（二層構造への変更後、`Remote` / `RemoteShared` / `Remoting` trait の新シグネチャで test が green になること）。
- [ ] 10.2 `rtk cargo test -p fraktor-remote-adaptor-std-rs` を実行し、green を確認する（installer の `RemoteShared` 化後、test が green になること）。
- [ ] 10.3 `rtk cargo test -p fraktor-cluster-adaptor-std-rs` を実行し、依存先の green を確認する（`Remoting` trait シグネチャ変更の波及を吸収）。
- [ ] 10.4 handshake / quarantine / watermark backpressure / instrument 通知 / handshake generation 破棄の integration test を追加または更新する。
  - `installer.install` → `installer.remote().start()` → `installer.spawn_run_task()` の起動順序を検証
  - `Remoting::quarantine` を run と並行して呼ぶケースも含めて検証（per-event lock の隙間で進行することを確認）
  - `installer.shutdown_and_join().await` で graceful shutdown が成立することを検証
  - `Remoting::shutdown` 単独呼び出し後、event 1 件で run loop が終了することも検証
- [ ] 10.5 showcase（`showcases/std/remote_lifecycle/` 等）が新 API で起動することを確認する。

## 11. 検証

- [x] 11.1 dylint（mod-file、module-wiring、type-per-file、tests-location、use-placement、rustdoc、cfg-std-forbid、ambiguous-suffix）を `rtk cargo clippy` 系で確認する。
- [x] 11.2 `rtk ./scripts/ci-check.sh ai all` を最後まで完了させる。
