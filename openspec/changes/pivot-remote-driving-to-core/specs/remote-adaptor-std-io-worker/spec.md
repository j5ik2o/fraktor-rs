## REMOVED Requirements

### Requirement: outbound loop (送信 tokio task)

**理由:** `RemoteShared::run`（内部で `Remote::handle_remote_event` を呼ぶ）が core 側で outbound 駆動を担当するため、adapter 側の `outbound_loop` は不要となる。`Remote::handle_remote_event` が `Association::next_outbound()` を呼び、`Codec::encode` を経て `RemoteTransport` に送信する経路に置き換えられる。

### Requirement: handshake driver (タイムアウト監視)

**理由:** handshake timeout は `AssociationEffect::StartHandshake { authority, timeout, generation }` を実行する際に adapter 側 I/O ワーカーが per-association tokio task として確保し、満了時に `RemoteEvent::HandshakeTimerFired { authority, generation }` を adapter 内部 sender 経由で receiver に push する形に置き換えられるため、`handshake_driver.rs` 単体としての独立 task ファイルは不要となる。

## MODIFIED Requirements

### Requirement: inbound dispatch (受信 I/O ワーカー)

adapter 側に受信 tokio task が定義され、TCP から受信した frame を `RemoteEvent::InboundFrameReceived { authority, frame }` として adapter 内部 sender 経由で `RemoteEventReceiver` に push する SHALL。`Association` の `handshake_accepted` 等を **直接呼んではならない**（MUST NOT）— state machine への反映は core 側の `Remote::handle_remote_event`（`RemoteShared::run` の `with_write` 区間内）が担当する。

#### Scenario: 受信 frame の event push

- **WHEN** 受信 loop が TCP frame を受信する
- **THEN** `RemoteEvent::InboundFrameReceived { authority, frame }` を構築する
- **AND** adapter 内部の sender（`tokio::sync::mpsc::Sender<RemoteEvent>` 等）に push し、`Result` を観測可能に扱う（`?` または `match`、`let _ = ...` での無言握りつぶしは禁止）

#### Scenario: Association 直接呼び出しの不在

- **WHEN** `modules/remote-adaptor-std/src/std/inbound_dispatch.rs` または同等のソースを検査する
- **THEN** `Association::handshake_accepted` / `accept_handshake_request` / `accept_handshake_response` 等の core state 遷移メソッドを直接呼ぶ箇所が存在しない

#### Scenario: monotonic 時刻入力の不要化

- **WHEN** inbound I/O ワーカーが core に時刻を渡すかどうかを検査する
- **THEN** I/O ワーカーは時刻を渡さない（時刻は `Remote::handle_remote_event` 側で adapter 提供の `now_provider` または `RemoteEvent` に同梱された値経由で取得する）
- **AND** wall clock の混入は発生しない

## ADDED Requirements

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

run task の wake と完了観測を 1 step で行う adapter 固有の async API `RemotingExtensionInstaller::shutdown_and_join(&self) -> impl Future<Output = Result<(), RemotingError>>` を提供する SHALL。**`&self` を取る**（`self` consume ではない、`ExtensionInstaller` で actor system に登録されたまま使えるようにする）。`RemoteShared::shutdown` は **wake せず**、`event_sender` を持たない（薄いラッパー原則）。

shutdown_and_join 内では must-use 戻り値を `let _ = ...` で握りつぶしてはならない（MUST NOT、`.agents/rules/ignored-return-values.md` 準拠）。失敗の意味分類と扱いを明示する。

#### Scenario: 同期 Remoting::shutdown の挙動（純デリゲートのみ）

- **WHEN** `Remoting::shutdown`（`RemoteShared::shutdown` 経由）が呼ばれる
- **THEN** `with_write(|remote| remote.shutdown())` で `Remote::shutdown` を呼び lifecycle を terminated に遷移する
- **AND** **wake はしない**（`RemoteShared` は `event_sender` を持たない）
- **AND** `event_sender.send(...).await` や `event_sender.try_send(...)` や `run_handle.await` を内部で実行しない（`RemoteShared` 自体が adapter 都合の field を持たないため）
- **AND** 同期 method なので `await` を内部で行わない

#### Scenario: shutdown_and_join の手順（&self、内部可変性経由）

- **WHEN** `installer.shutdown_and_join(&self).await` が呼ばれる
- **THEN** installer は次の 3 ステップを順次実行する
  1. `self.remote_shared.get().ok_or(RemotingError::NotStarted)?.shutdown()` を呼ぶ（lifecycle terminated 遷移、`RemoteShared::shutdown` の純デリゲート）。`Err(RemotingError::NotStarted)` のみ idempotent として `match` で無視（直前コメントで「すでに停止済み」と明記）、それ以外の `Err` は `?` で伝播
  2. `self.event_sender.get()` で `Sender` を取得し、`if let Err(send_err) = sender.try_send(RemoteEvent::TransportShutdown) { tracing::debug!(?send_err, "shutdown wake failed (best-effort)"); }` で wake（Full / Closed 失敗は log 記録）
  3. `self.run_handle.lock().map_err(...)?.take()` で `Option<JoinHandle>` から handle を取り出し、`Some(handle)` なら `handle.await` で run task の終了を観測する
- **AND** ステップ 3 の `Result<Result<(), RemotingError>, JoinError>` を `match` で全分岐扱い `Ok(Ok(())) → Ok(())` / `Ok(Err(e)) → Err(e)` / `Err(join_err) → tracing::error!(?join_err, "...") + Err(RemotingError::TransportUnavailable)`

#### Scenario: must-use 戻り値の握りつぶし禁止に従う

- **WHEN** `shutdown_and_join` 実装の `Result` 戻り値の扱いを検査する
- **THEN** `let _ = self.remote_shared...shutdown();` のような無言握りつぶしが存在しない
- **AND** ステップ 1 の `Result` は `match` で全分岐を扱う（`NotStarted` のみ idempotent でコメント付き許容、それ以外は伝播）
- **AND** ステップ 2 の `try_send` の `Result` は `if let Err(send_err) = ...` で error 値を log に渡す
- **AND** ステップ 3 の `JoinHandle::await` の `Result` は `match` で全分岐を扱い、`Err(JoinError)` は log 記録 + `RemotingError::TransportUnavailable` 変換

#### Scenario: try_send の Full / Closed の意味分類

- **WHEN** `event_sender.try_send(RemoteEvent::TransportShutdown)` が `Err(TrySendError::Full)` を返す
- **THEN** event channel が満杯であり、現在 receiver は未消費 event を保持している
- **AND** その未消費 event の処理後 `RemoteShared::run` が `is_terminated()` Query で `true` を観測してループ終了する
- **AND** ステップ 3 の `handle.await` で完了を観測できるため、wake 失敗は best-effort として log 記録に留める
- **WHEN** `event_sender.try_send(RemoteEvent::TransportShutdown)` が `Err(TrySendError::Closed)` を返す
- **THEN** receiver は既に drop しており、`RemoteShared::run` task は既に終了している（`EventReceiverClosed` 経由で）
- **AND** ステップ 3 の `handle.await` で `Err(EventReceiverClosed)` または既終了結果が観測されるため、wake 失敗は best-effort として log 記録に留める

#### Scenario: RemoteShared::run の異常終了の観測

- **WHEN** `RemoteShared::run` が `Err(RemotingError::TransportUnavailable)` 等を返した
- **THEN** `installer.shutdown_and_join(&self).await` のステップ 3 の戻り値で error が呼出元に伝播される
- **AND** adapter は必要に応じて log 記録 / actor system の error path への通知を行う

#### Scenario: shutdown_and_join 単独でも完了する

- **WHEN** 呼び出し側が事前に `Remoting::shutdown` を呼ばずに `installer.shutdown_and_join(&self).await` だけを呼ぶ
- **THEN** ステップ 1 の `RemoteShared::shutdown` で lifecycle 遷移が行われ、ステップ 2 の wake と ステップ 3 の完了観測が完結する
- **AND** 結果として graceful shutdown が成立する（呼び出し側が手順を意識する必要がない）

#### Scenario: 別 Driver 型の不在

- **WHEN** adapter 側の installer 実装を検査する
- **THEN** `RemoteDriverHandle` や `RemoteDriverOutcome` を import / 利用していない

### Requirement: tokio mpsc ベースの RemoteEventReceiver

adapter 側に `tokio::sync::mpsc` 受信側ラッパとして `RemoteEventReceiver` 実装が定義される SHALL。送信側 `Sender` は adapter 内部の I/O ワーカー / handshake timer task が clone して共有し、`RemoteEventSink` 等の trait としては core に公開しない。

#### Scenario: Receiver 実装の存在

- **WHEN** `modules/remote-adaptor-std/src/std/tokio_remote_event_receiver.rs` を読む
- **THEN** `pub struct TokioMpscRemoteEventReceiver` または同等の型が定義され、`impl RemoteEventReceiver` を持つ
- **AND** 内部で `tokio::sync::mpsc::Receiver<RemoteEvent>` を保持する

#### Scenario: Sink trait の不在

- **WHEN** `modules/remote-core/src/core/extension/` 配下のソースを検査する
- **THEN** `pub trait RemoteEventSink` または同等の trait が定義されていない（adapter 内部の `Sender` は core に公開せず、純増ゼロ方針を維持する）

#### Scenario: bounded / unbounded の選択

- **WHEN** Receiver 実装の channel 形式を検査する
- **THEN** bounded（背圧あり）か unbounded（背圧なし）かが docstring に明記されている
- **AND** 既定では bounded を選択する
- **AND** capacity の確定（固定値 / 別経路 / 既存 `RemoteConfig` 参照）は実装 PR で行う（design.md Open Questions 参照）。本 change では `RemoteConfig` に capacity 用の新フィールドを追加しない（純増ゼロ方針維持）

### Requirement: schedule_handshake_timeout は tokio task として timer を確保する

adapter は `RemoteTransport::schedule_handshake_timeout(authority, timeout, generation)`（`remote-core-transport-port` capability で要件化）の実装で、`tokio::time::sleep(timeout)` する task を spawn し、満了時に `RemoteEvent::HandshakeTimerFired { authority, generation }` を adapter 内部 sender 経由で receiver に push する SHALL。core 側に Timer Port を新設してはならない（MUST NOT）。

#### Scenario: schedule_handshake_timeout の発火経路

- **WHEN** `Remote::handle_remote_event` が `AssociationEffect::StartHandshake { authority, timeout, generation }` を実行し、`RemoteTransport::send_handshake` の後に `RemoteTransport::schedule_handshake_timeout(&authority, timeout, generation)` を呼ぶ
- **THEN** adapter は `tokio::spawn(async move { tokio::time::sleep(timeout).await; sender.send(RemoteEvent::HandshakeTimerFired { authority, generation }).await; })` 相当の経路で timer を確保する
- **AND** generation 値は呼出引数から受け取った値をそのまま timer task が保持する
- **AND** 戻り値の `Result<(), TransportError>` は spawn 成功で `Ok(())` を返す（sleep の満了は非同期に行われるため、本メソッドは sleep を await しない）

#### Scenario: Timer trait の不在

- **WHEN** `modules/remote-core/src/core/` 配下のソースを検査する
- **THEN** `pub trait Timer` や `pub struct TimerToken` が定義されていない（純増ゼロ方針）

#### Scenario: 古い timer の発火許容

- **WHEN** Association の状態が次の Handshaking に進み、generation が +1 された後に古い timer が満了する
- **THEN** adapter は generation 値を変更せずに `HandshakeTimerFired { generation: g_event }` を push する
- **AND** `Remote::handle_remote_event` 側で「current generation `!=` g_event なので破棄」する判定が行われる（adapter 側でのキャンセルは不要）
- **AND** 判定は `!=` 比較で行い、大小比較（`>` / `<`）は使わない（`wrapping_add` の wrap 時にも stale 判定が漏れないようにするため）

### Requirement: outbound enqueue は RemoteEvent::OutboundEnqueued として通知する

local actor が remote ref に tell する経路で、adapter は `OutboundEnvelope` を構築した後 `AssociationRegistry` を直接 mutate せず、`RemoteEvent::OutboundEnqueued { authority, envelope }` を adapter 内部 sender 経由で receiver に push する SHALL。`Remote::handle_remote_event` がこの event を受けて `Association::enqueue` と outbound drain を実行する（実行責務は `remote-core-extension` capability で要件化済）。

#### Scenario: enqueue 経路

- **WHEN** local actor の tell から adapter 側 RemoteActorRef 相当に到達する
- **THEN** adapter は `OutboundEnvelope` を構築する
- **AND** `RemoteEvent::OutboundEnqueued { authority, envelope }` を adapter 内部 sender 経由で push する
- **AND** `Result` を `?` または `match` で扱う（`let _ = ...` での無言握りつぶしは禁止）

#### Scenario: AssociationRegistry 直接操作の不在

- **WHEN** adapter 側の RemoteActorRef / dispatch 経路を検査する
- **THEN** `AssociationRegistry::*` の状態変更メソッド（`enqueue` / `next_outbound` 等）を直接呼ぶ箇所が存在しない
- **AND** `Association` の `&mut` アクセスは `Remote::handle_remote_event` 経由でのみ行われる

#### Scenario: bounded channel での enqueue 失敗

- **WHEN** adapter 内部 sender が bounded で、buffer 満杯時に `try_send` が失敗する
- **THEN** adapter は失敗を transport-level error として扱い、必要に応じて caller に伝播するか、metrics として記録する
- **AND** sender 飢餓時の挙動は実装 PR で確定する（本 change のスコープ外、design.md の Open Questions 参照）

### Requirement: 効果適用から StartHandshake 分岐を削除する

adapter 側の `effect_application::apply_effects_in_place`（または相当箇所）から `AssociationEffect::StartHandshake` の dispatch 分岐を削除する SHALL。`Remote::handle_remote_event` 側が `RemoteTransport` 経由で handshake を開始するため、adapter 側で同 effect を扱う必要がない。

#### Scenario: StartHandshake 分岐の不在

- **WHEN** `modules/remote-adaptor-std/src/std/effect_application.rs` を検査する
- **THEN** `AssociationEffect::StartHandshake { .. } =>` 分岐が存在しない
- **AND** unreachable! の使用も存在しない（`Remote::handle_remote_event` がすべて処理するため adapter のこの経路を通らない）

#### Scenario: 残存 effect の adapter 処理

- **WHEN** adapter 側で扱うべき effect 種別を検査する
- **THEN** `SendEnvelopes`、`DiscardEnvelopes`、`PublishLifecycle` 等の I/O 直結 effect のみが adapter 側で扱われ、状態遷移を伴う effect は `Remote::handle_remote_event` 側に集約される
