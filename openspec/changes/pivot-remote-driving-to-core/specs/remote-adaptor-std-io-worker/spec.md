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

### Requirement: Installer は RemoteShared を保持し外部公開する

`RemotingExtensionInstaller` は `RemoteShared` を field として保持し、`installer.remote() -> RemoteShared` で外部公開しなければならない（MUST）。raw `SharedLock<Remote>` / `Arc<Mutex<Remote>>` / `Arc<Remote>` を field として保持してはならない（MUST NOT）。`installer.remote()` は raw `SharedLock<Remote>` を返してはならない（MUST NOT、`RemoteShared` を返す）。

#### Scenario: install 時に RemoteShared を構築する

- **WHEN** `RemotingExtensionInstaller::install` を検査する
- **THEN** `Remote::with_instrument(transport, config, event_publisher, instrument)` で `Remote` を構築する
- **AND** 続けて `RemoteShared::new(remote)` で `RemoteShared` を構築し、installer の `remote_shared` field に保存する
- **AND** raw `SharedLock<Remote>` を field に直接保存しない

#### Scenario: spawn 経路は RemoteShared::run を clone から呼ぶ

- **WHEN** installer が `RemoteShared::run` を spawn する経路を検査する
- **THEN** `let run_target = self.remote_shared.clone();` した上で `tokio::spawn(async move { run_target.run(&mut event_receiver).await })` 相当で起動する
- **AND** spawn 後も installer は `remote_shared` field を保持し続け、外部から `installer.remote()` で取得できる
- **AND** `Remote` を spawn task に move する設計（`Remote::run(self, ..)`）を採らない

#### Scenario: raw shared Remote field の不在

- **WHEN** installer の field 構成を検査する
- **THEN** raw `Arc<Remote>` / `Mutex<Remote>` / `RwLock<Remote>` / `SharedLock<Remote>` field が存在しない
- **AND** `RemoteShared` を保持する `remote_shared: RemoteShared` field のみが許容される（その内部の `SharedLock<Remote>` は `RemoteShared` API でカプセル化される）
- **AND** `Remote` の field を installer 側から直接読み書きする経路が存在しない

#### Scenario: 公開 getter のシグネチャ

- **WHEN** `RemotingExtensionInstaller::remote` の戻り値型を検査する
- **THEN** `pub fn remote(&self) -> RemoteShared`（または `Result<RemoteShared, _>`）を返す
- **AND** raw `SharedLock<Remote>` を返す API は公開されていない

### Requirement: 外部制御は Remoting trait と Sender / JoinHandle で行う

run task に対する外部制御は次の手段で行う SHALL。

- `Remoting` trait（`RemoteShared` 実装）の `start` / `shutdown` / `quarantine` / `addresses` — 同期 method、すべて `&self`
- `Sender<RemoteEvent>`（adapter 内部 mpsc の送信側を installer が clone 保持）— `try_send(TransportShutdown)` で best-effort wake、I/O ワーカー / handshake timer task / RemoteActorRef が clone 共有
- `JoinHandle<Result<(), RemotingError>>`（spawn の戻り値）

raw `SharedLock<Remote>` を installer field として外部公開してはならない（MUST NOT、`RemoteShared` でカプセル化される）。

#### Scenario: installer の保持 field

- **WHEN** `RemotingExtensionInstaller` または同等型の field を検査する
- **THEN** `remote_shared: RemoteShared` / `event_sender: tokio::sync::mpsc::Sender<RemoteEvent>` / `run_handle: JoinHandle<Result<(), RemotingError>>` 程度のみを保持する
- **AND** `cached_addresses: Vec<Address>` のような addresses cache field を持たない（`RemoteShared::addresses` で source of truth から取得するため）
- **AND** `Remote` への raw 直接参照（`Arc<Remote>` / `&Remote` / raw `SharedLock<Remote>`）を保持しない

#### Scenario: addresses クエリは RemoteShared 経由で source of truth から返す

- **WHEN** `Remoting::addresses()`（`RemoteShared::addresses` 経由）が呼ばれる
- **THEN** `RemoteShared::addresses(&self)` が `with_read(|remote| remote.addresses().to_vec())` で内部 `Remote` から owned `Vec<Address>` を返す
- **AND** installer 側のキャッシュ field を経由しない

#### Scenario: 起動順序

- **WHEN** installer が `RemoteShared::run` を spawn する前後の処理を検査する
- **THEN** spawn 前に `remote_shared.start()`（`with_write(|r| r.start())`）相当を呼んで `Remote::start` を完了させ、transport の listening を確立する
- **AND** `Remote::addresses()` の戻り値が反映された後で spawn を行う（spawn 後も `RemoteShared::addresses` で常に最新が取れる）
- **AND** `Remote::start` 等の別 API は新設しない

### Requirement: run task の停止プロトコル

run task の完了完了を保証する経路は `Remoting` trait の外側、adapter 固有の async wait surface に分離する SHALL。同期 `Remoting::shutdown`（`RemoteShared::shutdown` 経由）の内部で `event_sender.send(...).await` や `run_handle.await` を実行してはならない（MUST NOT）。

#### Scenario: 同期 Remoting::shutdown の挙動

- **WHEN** `Remoting::shutdown`（`RemoteShared::shutdown` 経由）が呼ばれる
- **THEN** `with_write(|remote| remote.shutdown())` で `Remote::shutdown` を呼び lifecycle を terminated に遷移する
- **AND** 続けて `event_sender.try_send(RemoteEvent::TransportShutdown).ok()` 相当で best-effort wake する。`try_send` の失敗（Full / Closed）は無視可能（次の event 処理時に lifecycle が観測されてループは正常終了する）
- **AND** `event_sender.send(...).await` や `run_handle.await` を内部で実行しない（同期 method）

#### Scenario: async wait surface の手順

- **WHEN** adapter 固有の async wait surface が呼ばれる
- **THEN** （任意）`RemoteShared::shutdown` で停止要求を送る（同期、`await` 不要）
- **AND** 続けて `run_handle.await` で run task の終了（`Ok(())`）を観測する
- **AND** `JoinHandle` 結果が `Ok(Ok(()))` であれば正常終了
- **AND** `Ok(Err(e))` / `Err(join_err)` は `RemotingError` に変換して呼出元に伝播する
- **AND** `let _ = ...` による無言握りつぶしは存在しない

#### Scenario: RemoteShared::run の異常終了の観測

- **WHEN** `RemoteShared::run` が `Err(RemotingError::TransportUnavailable)` 等を返した
- **THEN** adapter wait surface は `JoinHandle::await` の戻り値で error を log に記録し、actor system の error path に伝播する

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
