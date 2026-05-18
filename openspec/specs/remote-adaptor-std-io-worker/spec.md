# remote-adaptor-std-io-worker Specification

## Purpose
TBD - created by archiving change remote-redesign. Update Purpose after archive.
## Requirements
### Requirement: inbound dispatch (受信 tokio task)

adapter 側に受信 tokio task が定義され、TCP から受信した core wire frame bytes を `RemoteEvent::InboundFrameReceived { authority, frame, now_ms }` として adapter 内部 sender 経由で `RemoteEventReceiver` に push する SHALL。`Association` の `handshake_accepted` 等を **直接呼んではならない**（MUST NOT）— state machine への反映は core 側の `Remote::handle_remote_event`（`RemoteShared::run` の `with_write` 区間内）が担当する。

#### Scenario: 受信 frame の event push

- **WHEN** 受信 loop が TCP frame を受信する
- **THEN** adapter は core wire frame bytes と adapter の monotonic clock から `RemoteEvent::InboundFrameReceived { authority, frame, now_ms }` を構築する
- **AND** adapter 内部の sender（`tokio::sync::mpsc::Sender<RemoteEvent>` 等）に push し、`Result` を観測可能に扱う（`?` または `match`、`let _ = ...` での無言握りつぶしは禁止）

#### Scenario: Association 直接呼び出しの不在

- **WHEN** `modules/remote-adaptor-std/src/inbound_dispatch.rs` または同等のソースを検査する
- **THEN** `Association::handshake_accepted` / `accept_handshake_request` / `accept_handshake_response` 等の core state 遷移メソッドを直接呼ぶ箇所が存在しない

#### Scenario: monotonic 時刻入力の event 同梱

- **WHEN** inbound I/O ワーカーが core に時刻を渡すかどうかを検査する
- **THEN** I/O ワーカーは `RemoteEvent` の `now_ms` として monotonic 時刻を同梱する
- **AND** wall clock の混入は発生しない

### Requirement: inbound local actor delivery bridge

adapter 側は、`Remote::handle_remote_event(InboundFrameReceived)` により buffer された `InboundEnvelope` を drain し、local actor system / provider へ配送する bridge を持たなければならない（MUST）。`remote-core` は actor mailbox delivery を直接行ってはならない（MUST NOT）。

#### Scenario: core event step 後に inbound envelopes を drain する

- **WHEN** remote run task が `RemoteEvent::InboundFrameReceived` を処理した
- **THEN** adapter は同じ event step の後に `RemoteShared::drain_inbound_envelopes()` または同等の core API を呼ぶ
- **AND** 返された `InboundEnvelope` を local delivery bridge に渡す
- **AND** drain と actor delivery は remote write lock を保持したまま実行しない

#### Scenario: recipient path を local actor に解決する

- **WHEN** delivery bridge が `InboundEnvelope` を受け取る
- **THEN** `recipient` の `ActorPath` を actor-core provider / actor system 経由で local `ActorRef` に解決する
- **AND** 解決できた場合は envelope payload を `AnyMessage` として local actor ref に送信する
- **AND** sender path が存在する場合は actor-core の既存 sender 表現に沿って保持または復元する

#### Scenario: delivery failure は観測可能である

- **WHEN** recipient が存在しない、mailbox が closed、または payload 復元に失敗する
- **THEN** adapter は actor-core の dead letters または明示的な adapter error path に失敗を流す
- **AND** 失敗 envelope を silent drop してはならない

### Requirement: run task orchestration は event ごとの delivery hook をサポートする

adapter の run task は、core event processing と inbound local delivery を同じ orchestration loop で接続できなければならない（MUST）。

#### Scenario: RemoteShared::run が hook できない場合の最小 core API

- **WHEN** `RemoteShared::run(&mut receiver).await` だけでは event ごとの後処理を挟めない
- **THEN** core は raw `SharedLock<Remote>` を露出しない最小 API を提供する
- **AND** その API は 1 件の `RemoteEvent` を `Remote::handle_remote_event` に委譲し、停止判定だけを adapter に返す
- **AND** adapter は `match event` や `Association` 直接操作を実装しない

#### Scenario: run task は shutdown semantics を維持する

- **WHEN** adapter-owned run loop が `TransportShutdown` を処理する
- **THEN** `RemoteShared` / `Remote` の既存 shutdown semantics に従い event loop を終了する
- **AND** `shutdown_and_join` は wake + `JoinHandle` 完了観測を引き続き提供する

### Requirement: RemoteSettings の ack フィールド追加

Phase A では延期された `RemoteSettings::ack_send_window` と `ack_receive_window` フィールドは、本 Phase (adapter side の system message delivery 実装) と同時に `remote-core-settings` capability に追加される SHALL。

#### Scenario: ack フィールドの追加

- **WHEN** Phase B 完了時点の `RemoteSettings` のフィールドを検査する
- **THEN** `ack_send_window: u64` と `ack_receive_window: u64` が追加されている

### Requirement: inbound restart budget

std adaptor の association runtime は `RemoteConfig` の inbound restart budget を参照し、inbound loop の再起動を deadline-window budget で制限する SHALL。budget 超過時は無限 restart を行わず、観測可能な失敗として呼び出し元または runtime の error path に返す MUST。

#### Scenario: inbound restart 設定を runtime に渡す

- **WHEN** std adaptor が `RemoteConfig` から association runtime を構築する
- **THEN** inbound restart timeout と inbound max restarts が inbound loop の restart policy に渡される

#### Scenario: budget 内の inbound restart を許可する

- **WHEN** inbound loop が restart timeout window 内で inbound max restarts 以下の回数だけ失敗する
- **THEN** association runtime は inbound loop の restart を許可する

#### Scenario: budget 超過時に inbound restart を停止する

- **WHEN** inbound loop が restart timeout window 内で inbound max restarts を超えて失敗する
- **THEN** association runtime は追加 restart を行わず、失敗を観測可能な error path に返す

#### Scenario: restart window は monotonic time で判定する

- **WHEN** association runtime が inbound restart timeout window を評価する
- **THEN** `Instant` ベースの monotonic millis を使い、`SystemTime` などの wall clock に依存しない

### Requirement: advanced settings do not imply Pekko wire compatibility

std adaptor は `RemoteConfig` の large-message / compression advanced settings を参照可能にする SHALL。ただし、この change では Pekko Artery TCP framing、protobuf control PDU、compression table の byte-compatible な送受信を実装しない MUST。

#### Scenario: large-message 設定は wire codec を変更しない

- **WHEN** large-message destinations または outbound large-message queue size が設定されている
- **THEN** std adaptor は既存の fraktor-rs wire codec を維持し、Pekko Artery TCP framing を生成しない

#### Scenario: compression 設定は wire codec を変更しない

- **WHEN** compression settings が設定されている
- **THEN** std adaptor は compression table advertisement や Pekko protobuf control PDU を送信せず、設定値を後続 protocol 実装の入力として保持する

### Requirement: Installer は RemoteShared を保持し外部公開する（&self + 内部可変性）

`RemotingExtensionInstaller` は **`Send + Sync + 'static`** で actor system に登録される `ExtensionInstaller`。`install(&self, system: &ActorSystem) -> Result<(), ActorSystemBuildError>` 契約に従うため、書き換え可能 field は **内部可変性で包まなければならない**（MUST、`OnceLock` / `Mutex<Option<_>>`）。`RemoteShared` を保持し、`installer.remote() -> Result<RemoteShared, _>` で外部公開しなければならない（MUST）。raw `SharedLock<Remote>` / `Arc<Mutex<Remote>>` / `Arc<Remote>` を field として保持してはならない（MUST NOT）。`installer.remote()` は raw `SharedLock<Remote>` を返してはならない（MUST NOT、`RemoteShared` を返す）。

#### Scenario: installer field 構成（内部可変性）

- **WHEN** `RemotingExtensionInstaller` の field 構成を検査する
- **THEN** 次の field 構成を持つ
  - `transport: std::sync::Mutex<Option<TcpRemoteTransport>>`（install で take）
  - `config: RemoteConfig`（構築後 immutable）
  - `remote_shared: std::sync::OnceLock<RemoteShared>`（install で 1 回だけ set）
  - `event_sender: std::sync::OnceLock<tokio::sync::mpsc::Sender<RemoteEvent>>`（install で 1 回だけ set）
  - `event_receiver: std::sync::Mutex<Option<TokioMpscRemoteEventReceiver>>`（spawn_run_task で take）
  - `run_handle: std::sync::Mutex<Option<JoinHandle<Result<(), RemotingError>>>>`（spawn_run_task で set、shutdown_and_join で take）
- **AND** raw `Arc<Remote>` / `Mutex<Remote>` / `RwLock<Remote>` / `SharedLock<Remote>` / `RemoteShared`（`OnceLock` で包まない裸の field）が存在しない
- **AND** `cached_addresses: Vec<Address>` のような addresses cache field を持たない

#### Scenario: install(&self, system) の挙動

- **WHEN** `impl ExtensionInstaller for RemotingExtensionInstaller` の `install(&self, system: &ActorSystem)` を検査する
- **THEN** `transport` field の `Mutex` を取り、`Option::take()` で transport を取り出す（既に take 済 / `OnceLock` set 済なら `ALREADY_INSTALLED` エラー）
- **AND** `Remote::with_instrument(transport, self.config.clone(), event_publisher, ...)` で `Remote` を構築 → `RemoteShared::new(remote)` で `RemoteShared` を構築
- **AND** `tokio::sync::mpsc::channel(capacity)` で channel を作成し、`OnceLock::set(sender)` / `Mutex<Option<_>>` への `Some(receiver)` 設定を行う
- **AND** `OnceLock::set(remote_shared)` で `remote_shared` field を初期化（重複 install は `ALREADY_INSTALLED` エラー）
- **AND** `Remote::start` を呼ばない（外部から `installer.remote()?.start()` で呼ぶ）
- **AND** run task を `tokio::spawn` で起動しない（明示 API `installer.spawn_run_task(&self)` を別途呼ぶ）

#### Scenario: spawn_run_task(&self) の挙動

- **WHEN** `installer.spawn_run_task(&self) -> Result<(), RemotingError>` が呼ばれる
- **THEN** 戻り値型のシグネチャが `&self`（`&mut self` ではない）
- **AND** `event_receiver.lock()?.take()` で `Option<TokioMpscRemoteEventReceiver>` から receiver を取り出す（既に take 済なら `RemotingError::AlreadyRunning`）
- **AND** `remote_shared.get().cloned().ok_or(RemotingError::NotStarted)?` で `RemoteShared` clone を取得
- **AND** `tokio::spawn(async move { run_target.run(&mut receiver).await })` で起動し、戻り値の `JoinHandle` を `run_handle.lock()? = Some(handle)` で保存
- **AND** spawn 後も installer は `remote_shared` を保持し続け、外部から `installer.remote()` で取得できる

#### Scenario: 公開 getter のシグネチャ

- **WHEN** `RemotingExtensionInstaller::remote` の戻り値型を検査する
- **THEN** `pub fn remote(&self) -> Result<RemoteShared, RemotingError>` を返す
- **AND** 内部実装は `self.remote_shared.get().cloned().ok_or(RemotingError::NotStarted)`
- **AND** raw `SharedLock<Remote>` を返す API は公開されていない

### Requirement: 外部制御は Remoting trait と Sender / JoinHandle で行う

run task に対する外部制御は次の手段で行う SHALL。

- `Remoting` trait（`RemoteShared` 実装）の `start` / `shutdown` / `quarantine` / `addresses` — 同期 method、すべて `&self`
- `Sender<RemoteEvent>`（adapter 内部 mpsc の送信側を installer が `OnceLock` で保持）— `try_send(TransportShutdown)` で best-effort wake、I/O ワーカー / handshake timer task / RemoteActorRef が clone 共有
- `JoinHandle<Result<(), RemotingError>>`（installer が `Mutex<Option<_>>` で保持）

raw `SharedLock<Remote>` を installer field として外部公開してはならない（MUST NOT、`RemoteShared` でカプセル化される）。

#### Scenario: addresses クエリは RemoteShared 経由で source of truth から返す

- **WHEN** `Remoting::addresses()`（`RemoteShared::addresses` 経由）が呼ばれる
- **THEN** `RemoteShared::addresses(&self)` が `with_read(|remote| remote.addresses().to_vec())` で内部 `Remote` から owned `Vec<Address>` を返す
- **AND** installer 側のキャッシュ field を経由しない

### Requirement: shutdown_and_join による wake + 完了観測（&self、握りつぶし禁止に従う）

run task の graceful flush、wake、完了観測を 1 step で行う adapter 固有の async API `RemotingExtensionInstaller::shutdown_and_join(&self) -> impl Future<Output = Result<(), RemotingError>>` を提供する SHALL。**`&self` を取る**（`self` consume ではない、`ExtensionInstaller` で actor system に登録されたまま使えるようにする）。`RemoteShared::shutdown` は **wake せず**、`event_sender` を持たない（薄いラッパー原則）。

`shutdown_and_join` は active association の shutdown flush 完了または timeout を待ってから `RemoteShared::shutdown` を呼ばなければならない（MUST）。shutdown flush の送信失敗または timeout は観測可能に記録しなければならないが（MUST）、transport shutdown と run task join を永久に止めてはならない（MUST NOT）。

shutdown_and_join 内では must-use 戻り値を `let _ = ...` で握りつぶしてはならない（MUST NOT、`.agents/rules/ignored-return-values.md` 準拠）。失敗の意味分類と扱いを明示する。

#### Scenario: 同期 Remoting::shutdown の挙動（純デリゲートのみ）

- **WHEN** `Remoting::shutdown`（`RemoteShared::shutdown` 経由）が呼ばれる
- **THEN** `with_write(|remote| remote.shutdown())` で `Remote::shutdown` を呼び lifecycle を terminated に遷移する
- **AND** **wake はしない**（`RemoteShared` は `event_sender` を持たない）
- **AND** `event_sender.send(...).await` や `event_sender.try_send(...)` や `run_handle.await` を内部で実行しない（`RemoteShared` 自体が adapter 都合の field を持たないため）
- **AND** 同期 method なので `await` を内部で行わない

#### Scenario: shutdown_and_join の手順（flush first、&self、内部可変性経由）

- **WHEN** `installer.shutdown_and_join(&self).await` が呼ばれる
- **THEN** installer は次の 5 ステップを順次実行する
  1. `remote_shared`、`event_sender`、flush timeout、run state を `&self` の内部可変性 field から取得する
  2. active association がある場合、association outbound queue を drain したうえで scope `Shutdown` の flush を request し、すべての flush completed または `shutdown_flush_timeout` まで待つ
  3. `self.remote_shared.get().ok_or(RemotingError::NotStarted)?.shutdown()` を呼ぶ（lifecycle terminated 遷移、`RemoteShared::shutdown` の純デリゲート。既に停止要求済みまたは停止済みなら no-op `Ok(())`）
  4. `self.event_sender.get()` で `Sender` を取得し、`if let Err(send_err) = sender.try_send(RemoteEvent::TransportShutdown) { tracing::debug!(?send_err, "shutdown wake failed (best-effort)"); }` で wake（Full / Closed 失敗は log 記録。`TransportShutdown` handler は既に停止要求済み/停止済みなら no-op）
  5. `self.run_handle.lock().map_err(...)?.take()` で `Option<JoinHandle>` から handle を取り出し、`Some(handle)` なら `handle.await` で run task の終了を観測する
- **AND** ステップ 5 の `Result<Result<(), RemotingError>, JoinError>` を `match` で全分岐扱い `Ok(Ok(())) → Ok(())` / `Ok(Err(e)) → Err(e)` / `Err(join_err) → tracing::error!(?join_err, "...") + Err(RemotingError::TransportUnavailable)`

#### Scenario: no active association skips flush wait

- **WHEN** `shutdown_and_join(&self).await` が呼ばれ、active association が存在しない
- **THEN** installer は shutdown flush request を送らない
- **AND** `RemoteShared::shutdown`、wake、join の手順へ進む

#### Scenario: shutdown flush timeout still shuts down

- **WHEN** shutdown flush が `shutdown_flush_timeout` までに完了しない
- **THEN** timeout は log または test-observable path に記録される
- **AND** installer は `RemoteShared::shutdown`、wake、join の手順へ進む

#### Scenario: must-use 戻り値の握りつぶし禁止に従う

- **WHEN** `shutdown_and_join` 実装の `Result` 戻り値の扱いを検査する
- **THEN** `let _ = self.remote_shared...shutdown();` のような無言握りつぶしが存在しない
- **AND** flush request / wait の `Result` は `match` または `?` で扱い、timeout / failure は log または returned outcome に残す
- **AND** ステップ 3 の `Result` は `?` または `match` で扱い、`Err` を idempotent として握りつぶす分岐は存在しない（既に停止要求済み/停止済みなら `RemoteShared::shutdown` 自体が no-op `Ok(())` を返す）
- **AND** ステップ 4 の `try_send` の `Result` は `if let Err(send_err) = ...` で error 値を log に渡す
- **AND** ステップ 5 の `JoinHandle::await` の `Result` は `match` で全分岐を扱い、`Err(JoinError)` は log 記録 + `RemotingError::TransportUnavailable` 変換

#### Scenario: try_send の Full / Closed の意味分類

- **WHEN** `event_sender.try_send(RemoteEvent::TransportShutdown)` が `Err(TrySendError::Full)` を返す
- **THEN** event channel が満杯であり、現在 receiver は未消費 event を保持している
- **AND** その未消費 event の処理後 `RemoteShared::run` が `is_terminated()` Query で `true` を観測してループ終了する
- **AND** ステップ 5 の `handle.await` で完了を観測できるため、wake 失敗は best-effort として log 記録に留める
- **WHEN** `event_sender.try_send(RemoteEvent::TransportShutdown)` が `Err(TrySendError::Closed)` を返す
- **THEN** receiver は既に drop しており、`RemoteShared::run` task は既に終了している（`EventReceiverClosed` 経由で）
- **AND** ステップ 5 の `handle.await` で `Err(EventReceiverClosed)` または既終了結果が観測されるため、wake 失敗は best-effort として log 記録に留める

#### Scenario: RemoteShared::run の異常終了の観測

- **WHEN** `RemoteShared::run` が `Err(RemotingError::TransportUnavailable)` 等を返した
- **THEN** `installer.shutdown_and_join(&self).await` のステップ 5 の戻り値で error が呼出元に伝播される
- **AND** adapter は必要に応じて log 記録 / actor system の error path への通知を行う

#### Scenario: shutdown_and_join 単独でも完了する

- **WHEN** 呼び出し側が事前に `Remoting::shutdown` を呼ばずに `installer.shutdown_and_join(&self).await` だけを呼ぶ
- **THEN** ステップ 2 の shutdown flush wait、ステップ 3 の `RemoteShared::shutdown`、ステップ 4 の wake、ステップ 5 の完了観測が完結する
- **AND** 結果として graceful shutdown が成立する（呼び出し側が手順を意識する必要がない）

#### Scenario: 別 Driver 型の不在

- **WHEN** adapter 側の installer 実装を検査する
- **THEN** `RemoteDriverHandle` や `RemoteDriverOutcome` を import / 利用していない

### Requirement: tokio mpsc ベースの RemoteEventReceiver

adapter 側に `tokio::sync::mpsc` 受信側ラッパとして `RemoteEventReceiver` 実装が定義される SHALL。送信側 `Sender` は adapter 内部の I/O ワーカー / handshake timer task が clone して共有し、`RemoteEventSink` 等の trait としては core に公開しない。

#### Scenario: Receiver 実装の存在

- **WHEN** `modules/remote-adaptor-std/src/tokio_remote_event_receiver.rs` を読む
- **THEN** `pub struct TokioMpscRemoteEventReceiver` または同等の型が定義され、`impl RemoteEventReceiver` を持つ
- **AND** 内部で `tokio::sync::mpsc::Receiver<RemoteEvent>` を保持する
- **AND** `RemoteEventReceiver::poll_recv` 実装は `tokio::sync::mpsc::Receiver::poll_recv` へ委譲する（core 側 trait に `async fn recv` は存在しない）

#### Scenario: Sink trait の不在

- **WHEN** `modules/remote-core/src/extension/` 配下のソースを検査する
- **THEN** `pub trait RemoteEventSink` または同等の trait が定義されていない（adapter 内部の `Sender` は core に公開せず、純増ゼロ方針を維持する）

#### Scenario: bounded / unbounded の選択

- **WHEN** Receiver 実装の channel 形式を検査する
- **THEN** bounded（背圧あり）か unbounded（背圧なし）かが docstring に明記されている
- **AND** 既定では bounded を選択する
- **AND** capacity の確定（固定値 / 別経路 / 既存 `RemoteConfig` 参照）は実装 PR で行う（design.md Open Questions 参照）。本 change では `RemoteConfig` に capacity 用の新フィールドを追加しない（純増ゼロ方針維持）

### Requirement: schedule_handshake_timeout は tokio task として timer を確保する

adapter は `RemoteTransport::schedule_handshake_timeout(authority, timeout, generation)`（`remote-core-transport-port` capability で要件化）の実装で、`tokio::time::sleep(timeout)` する task を spawn し、満了時に `RemoteEvent::HandshakeTimerFired { authority, generation, now_ms }` を adapter 内部 sender 経由で receiver に push する SHALL。core 側に Timer Port を新設してはならない（MUST NOT）。

#### Scenario: schedule_handshake_timeout の発火経路

- **WHEN** `Remote::handle_remote_event` が `AssociationEffect::StartHandshake { authority, timeout, generation }` を実行し、`RemoteTransport::send_handshake` の後に `RemoteTransport::schedule_handshake_timeout(&authority, timeout, generation)` を呼ぶ
- **THEN** adapter は `tokio::spawn(async move { tokio::time::sleep(timeout).await; let now_ms = monotonic_millis(); sender.send(RemoteEvent::HandshakeTimerFired { authority, generation, now_ms }).await })` 相当の経路で timer を確保する
- **AND** generation 値は呼出引数から受け取った値をそのまま timer task が保持する
- **AND** 戻り値の `Result<(), TransportError>` は spawn 成功で `Ok(())` を返す（sleep の満了は非同期に行われるため、本メソッドは sleep を await しない）

#### Scenario: Timer trait の不在

- **WHEN** `modules/remote-core/src/` 配下のソースを検査する
- **THEN** `pub trait Timer` や `pub struct TimerToken` が定義されていない（純増ゼロ方針）

#### Scenario: 古い timer の発火許容

- **WHEN** Association の状態が次の Handshaking に進み、generation が +1 された後に古い timer が満了する
- **THEN** adapter は generation 値を変更せずに `HandshakeTimerFired { generation: g_event }` を push する
- **AND** `Remote::handle_remote_event` 側で「current generation `!=` g_event なので破棄」する判定が行われる（adapter 側でのキャンセルは不要）
- **AND** 判定は `!=` 比較で行い、大小比較（`>` / `<`）は使わない（`wrapping_add` の wrap 時にも stale 判定が漏れないようにするため）

### Requirement: outbound enqueue は RemoteEvent::OutboundEnqueued として通知する

local actor が remote ref に tell する経路で、adapter は `OutboundEnvelope` を構築した後 `Association` を直接 mutate せず、`RemoteEvent::OutboundEnqueued { authority, envelope: Box<OutboundEnvelope>, now_ms }` を adapter 内部 sender 経由で receiver に push する SHALL。`Remote::handle_remote_event` がこの event を受けて `Association::enqueue(*envelope, now_ms)` と outbound drain を実行する（実行責務は `remote-core-extension` capability で要件化済）。

#### Scenario: enqueue 経路

- **WHEN** local actor の tell から adapter 側 RemoteActorRef 相当に到達する
- **THEN** adapter は `OutboundEnvelope` を構築する
- **AND** adapter の monotonic clock から `now_ms` を取得し、`RemoteEvent::OutboundEnqueued { authority, envelope: Box::new(envelope), now_ms }` を adapter 内部 sender 経由で push する
- **AND** `ActorRefSender::send` は同期 method のため `send(...).await` を使わず、`try_send` または `SendOutcome::Schedule` で async 境界へ渡す
- **AND** `Result` を `?` または `match` で扱う（`let _ = ...` での無言握りつぶしは禁止）。`try_send` の `Full` / `Closed` は caller が観測できる `SendError` 等へ変換する

#### Scenario: Association 直接操作の不在

- **WHEN** adapter 側の RemoteActorRef / dispatch 経路を検査する
- **THEN** `Association` の状態変更メソッド（`enqueue` / `next_outbound` 等）を直接呼ぶ箇所が存在しない
- **AND** `Association` の `&mut` アクセスは `Remote::handle_remote_event` 経由でのみ行われる

#### Scenario: bounded channel での enqueue 失敗

- **WHEN** adapter 内部 sender が bounded で、buffer 満杯時に `try_send` が失敗する
- **THEN** adapter は失敗を caller が観測できる `SendError` 等に変換して返す
- **AND** 補助的な metrics/log を追加する場合でも、主たる失敗を握りつぶしてはならない

### Requirement: actor-core provider 経由の remote sender 契約

adapter は actor-core の `ActorRefProvider` surface から remote path を解決した `ActorRef` について、送信時に `RemoteEvent::OutboundEnqueued` を adapter 内部 sender へ push する remote sender を提供する SHALL。cluster-* はこの provider surface 経由で remote actor ref を取得するため、本 requirement は cluster 固有の integration を持ち込まず remote-adaptor 側の利用契約だけを固定する。

#### Scenario: remote path 解決後の ActorRef sender

- **WHEN** actor-core が `StdRemoteActorRefProvider` 相当に remote authority を持つ `ActorPath` の解決を依頼する
- **THEN** provider は remote path 用の `ActorRef` を返す
- **AND** その sender は `RemoteActorRefSender` 相当であり、`ActorRefSender::send` 呼び出し時に `OutboundEnvelope` を構築する
- **AND** 構築した envelope は `RemoteEvent::OutboundEnqueued { authority, envelope: Box::new(envelope), now_ms }` として adapter 内部 sender に同期 push される
- **AND** push 失敗は caller が観測できる `SendError` 等へ変換される

#### Scenario: cluster 固有 integration は本 capability に含めない

- **WHEN** 本 change の acceptance を定義する
- **THEN** `ClusterApi::get` / `GrainRef` / cluster topology 更新の end-to-end integration test は要求しない
- **AND** それらは remote 側契約完成後の追加 change で扱う
- **AND** 本 change では `fraktor-cluster-adaptor-std-rs` の既存テスト green により `Remoting` trait 変更の波及だけを確認する

### Requirement: 効果適用から StartHandshake 分岐を削除する

adapter 側の `effect_application::apply_effects_in_place`（または相当箇所）から `AssociationEffect::StartHandshake` の dispatch 分岐を削除する SHALL。`Remote::handle_remote_event` 側が `RemoteTransport` 経由で handshake を開始するため、adapter 側で同 effect を扱う必要がない。

#### Scenario: StartHandshake 分岐の不在

- **WHEN** `modules/remote-adaptor-std/src/effect_application.rs` を検査する
- **THEN** `AssociationEffect::StartHandshake { .. } =>` 分岐が存在しない
- **AND** unreachable! の使用も存在しない（`Remote::handle_remote_event` がすべて処理するため adapter のこの経路を通らない）

#### Scenario: 残存 effect の adapter 処理

- **WHEN** adapter 側で扱うべき effect 種別を検査する
- **THEN** `SendEnvelopes`、`DiscardEnvelopes`、`PublishLifecycle` 等の I/O 直結 effect のみが adapter 側で扱われ、状態遷移を伴う effect は `Remote::handle_remote_event` 側に集約される

### Requirement: std watcher task applies WatcherEffect

std adaptor は `WatcherState` を所有して timer と command queue で駆動する watcher task を持つ SHALL。task は `WatcherEffect` を transport control、remote system message、actor-core DeathWatch delivery、event stream notification のいずれかへ変換する。

#### Scenario: heartbeat effect sends control pdu

- **WHEN** `WatcherState::handle` が `WatcherEffect::SendHeartbeat { to }` を返す
- **THEN** watcher task は `ControlPdu::Heartbeat` を対象 remote node へ送る
- **AND** 送信失敗は log または returned error path で観測できる

#### Scenario: watch effect enqueues system priority envelope

- **WHEN** `WatcherState::handle` が remote `Watch` system message の送信 effect を返す
- **THEN** watcher task は target actor path を recipient、watcher actor path を sender metadata とする system priority envelope を enqueue する
- **AND** envelope は ACK/NACK redelivery state の対象になる

#### Scenario: notify terminated sends actor-core system message

- **WHEN** `WatcherState::handle` が `NotifyTerminated { target, watchers }` を返す
- **THEN** watcher task は各 local watcher へ `SystemMessage::DeathWatchNotification(target_pid)` を送る
- **AND** target pid は remote actor path から local actor system 上の remote actor ref pid へ解決される

#### Scenario: quarantine notification is observable

- **WHEN** `WatcherState::handle` が `NotifyQuarantined { node }` を返す
- **THEN** watcher task は actor-core event stream または明示 error path に remote node quarantine を通知する
- **AND** notification を silent drop しない

### Requirement: inbound remote system message path rehydrates local pid

std inbound delivery bridge は remote DeathWatch 系 system message を actor-core へ渡す前に、envelope の actor path metadata から受信側 actor system の pid を解決する SHALL。wire 上の送信元 node local pid を actor-core にそのまま渡してはならない（MUST NOT）。

#### Scenario: inbound watch resolves remote watcher pid

- **GIVEN** remote node から recipient `target_path`、sender `watcher_path` を持つ `Watch` system envelope を受信した
- **WHEN** inbound delivery bridge が actor-core へ配送する
- **THEN** bridge は `watcher_path` を受信側の remote actor ref pid へ materialize または解決する
- **AND** `target_path` の local actor へ `SystemMessage::Watch(resolved_watcher_pid)` を送る

#### Scenario: inbound unwatch resolves remote watcher pid

- **GIVEN** remote node から recipient `target_path`、sender `watcher_path` を持つ `Unwatch` system envelope を受信した
- **WHEN** inbound delivery bridge が actor-core へ配送する
- **THEN** bridge は `watcher_path` を受信側の remote actor ref pid へ materialize または解決する
- **AND** `target_path` の local actor へ `SystemMessage::Unwatch(resolved_watcher_pid)` を送る

#### Scenario: inbound deathwatch notification resolves local watcher

- **GIVEN** remote node から recipient `watcher_path`、sender `target_path` を持つ `DeathWatchNotification` system envelope を受信した
- **WHEN** inbound delivery bridge が actor-core へ配送する
- **THEN** bridge は `watcher_path` を local actor pid へ解決する
- **AND** `target_path` を受信側の remote actor ref pid へ materialize または解決する
- **AND** local watcher へ `SystemMessage::DeathWatchNotification(resolved_target_pid)` を送る

### Requirement: retry driver uses core ACK/NACK effects

std retry driver は core association が返す ACK/NACK / resend effects を実行する SHALL。sequence state は std 側で二重に持ってはならない（MUST NOT）。

#### Scenario: resend effect sends retained system envelope

- **WHEN** core association が sequence number 付き system envelope の resend effect を返す
- **THEN** retry driver は同じ remote authority へ同じ system priority envelope を再送する
- **AND** retry driver は新しい sequence number を割り当てない

#### Scenario: ack pdu is routed into association

- **WHEN** TCP inbound dispatch が `AckPdu` を受信する
- **THEN** std run loop は `Remote::handle_remote_event` 経由で core association へ ACK を適用する
- **AND** ACK 後に返った resend / drop effects を retry driver が実行する

#### Scenario: retry timer is monotonic

- **WHEN** retry driver が pending system envelope の resend timeout を判定する
- **THEN** driver は monotonic millis を core に渡す
- **AND** wall clock に依存しない

### Requirement: remote-bound DeathWatch notification waits for flush outcome

std flush gate は remote watch hook から渡された remote-bound `DeathWatchNotification` を送る前に、対象 association の `BeforeDeathWatchNotification` flush を開始し、flush completed / timed out / failed のいずれかを観測してから notification envelope を enqueue する SHALL。remote-bound notification の発生点は `StdRemoteWatchHook::handle_deathwatch_notification` であり、`WatcherState` の heartbeat / failure detector 経路ではない。

#### Scenario: notification is delayed until flush completes

- **WHEN** remote watch hook が remote-bound `DeathWatchNotification` を std flush gate に渡す
- **THEN** flush gate は notification envelope を pending map に保持する
- **AND** 対象 association に `BeforeDeathWatchNotification` flush を request する
- **AND** flush completed を観測するまで notification envelope を enqueue しない

#### Scenario: timeout releases pending notification

- **GIVEN** remote-bound `DeathWatchNotification` が flush completion を待っている
- **WHEN** flush timeout を観測する
- **THEN** flush gate は timeout を log または test-observable path に記録する
- **AND** pending notification envelope を system priority envelope として enqueue する

#### Scenario: flush start failure releases pending notification

- **WHEN** flush gate が `BeforeDeathWatchNotification` flush を開始できない
- **THEN** failure を log または test-observable path に記録する
- **AND** notification envelope を破棄せず、system priority envelope として enqueue する

#### Scenario: completed flush enqueues exactly once

- **GIVEN** remote-bound `DeathWatchNotification` が flush completion を待っている
- **WHEN** flush completed event を複数回観測する
- **THEN** flush gate は notification envelope を一度だけ enqueue する

### Requirement: flush outcomes are applied after core event steps

std run loop は core event step が発生させた flush completed / timed-out / failed outcome を event step 後処理として std waiter / flush gate へ渡す SHALL。std waiter wake、pending notification release、actor-core enqueue の実行中に `Remote` の write lock を保持してはならない（MUST NOT）。

`RemoteTransport` は現行設計上 `Remote` が所有するため、flush request の transport 送信は `Remote::handle_remote_event` / `RemoteShared` の write lock 内で実行してよい。ただしこの transport method は bounded return / non-reentry 制約を守り、std の async wait や actor-core delivery を行ってはならない（MUST NOT）。

#### Scenario: flush request effect is sent through transport

- **WHEN** core event step が flush request effect を返す
- **THEN** core は `RemoteTransport` の lane-targeted flush request method へ flush request control frames を渡す
- **AND** send failure は log または returned error path に残す
- **AND** transport method は async wait、actor-core delivery、`RemoteShared` 再入を行わない

#### Scenario: flush completion wakes waiting shutdown

- **WHEN** core event step が shutdown flush completed effect を返す
- **THEN** std run loop は write lock を解放した後に `shutdown_and_join` の flush waiter を起こす
- **AND** waiter は `RemoteShared::shutdown` へ進める

#### Scenario: DeathWatch flush outcome wakes flush gate

- **WHEN** core event step が `BeforeDeathWatchNotification` flush completed または timed-out effect を返す
- **THEN** std run loop は write lock を解放した後に std flush gate へ flush outcome を渡す
- **AND** flush gate は pending notification の enqueue 判定を行う
