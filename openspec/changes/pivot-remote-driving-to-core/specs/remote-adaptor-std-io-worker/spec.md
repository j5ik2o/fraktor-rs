## REMOVED Requirements

### Requirement: outbound loop (送信 tokio task)

**理由:** `Remote::run` が core 側で outbound 駆動を担当するため、adapter 側の `outbound_loop` は不要となる。`Remote::run` が `Association::next_outbound()` を呼び、`Codec::encode` を経て `RemoteTransport` に送信する経路に置き換えられる。

### Requirement: handshake driver (タイムアウト監視)

**理由:** handshake timeout は `AssociationEffect::StartHandshake { authority, timeout, generation }` を実行する際に adapter 側 I/O ワーカーが per-association tokio task として確保し、満了時に `RemoteEvent::HandshakeTimerFired { authority, generation }` を adapter 内部 sender 経由で receiver に push する形に置き換えられるため、`handshake_driver.rs` 単体としての独立 task ファイルは不要となる。

## MODIFIED Requirements

### Requirement: inbound dispatch (受信 I/O ワーカー)

adapter 側に受信 tokio task が定義され、TCP から受信した frame を `RemoteEvent::InboundFrameReceived { authority, frame }` として adapter 内部 sender 経由で `RemoteEventReceiver` に push する SHALL。`Association` の `handshake_accepted` 等を **直接呼んではならない**（MUST NOT）— state machine への反映は core 側の `Remote::run` が担当する。

#### Scenario: 受信 frame の event push

- **WHEN** 受信 loop が TCP frame を受信する
- **THEN** `RemoteEvent::InboundFrameReceived { authority, frame }` を構築する
- **AND** adapter 内部の sender（`tokio::sync::mpsc::Sender<RemoteEvent>` 等）に push し、`Result` を観測可能に扱う（`?` または `match`、`let _ = ...` での無言握りつぶしは禁止）

#### Scenario: Association 直接呼び出しの不在

- **WHEN** `modules/remote-adaptor-std/src/std/inbound_dispatch.rs` または同等のソースを検査する
- **THEN** `Association::handshake_accepted` / `accept_handshake_request` / `accept_handshake_response` 等の core state 遷移メソッドを直接呼ぶ箇所が存在しない

#### Scenario: monotonic 時刻入力の不要化

- **WHEN** inbound I/O ワーカーが core に時刻を渡すかどうかを検査する
- **THEN** I/O ワーカーは時刻を渡さない（時刻は `Remote::run` 側で adapter 提供の `now_provider` または `RemoteEvent` に同梱された値経由で取得する）
- **AND** wall clock の混入は発生しない

## ADDED Requirements

### Requirement: Remote の所有権を run task に move する

`RemotingExtensionInstaller` は `Remote` インスタンスの所有権を spawn した tokio task に **move** しなければならない（MUST）。`Arc<Mutex<Remote>>` / `Arc<RwLock<Remote>>` / `SharedLock<Remote>`（`utils-core::SharedLock<T>`、旧 `AShared` パターンの実装実体）等の共有可変性で `Remote` を保持してはならない（MUST NOT）。

#### Scenario: 所有権 move の経路

- **WHEN** installer が `Remote::run` を spawn する経路を検査する
- **THEN** `tokio::spawn(async move { remote.run(&mut receiver).await })` 相当で `remote` 変数が move 入りする
- **AND** spawn 後、installer 側に `Remote` への参照が残らない

#### Scenario: 共有可変性の不在

- **WHEN** installer の field 構成を検査する
- **THEN** `Arc<Mutex<Remote>>` / `Arc<RwLock<Remote>>` / `SharedLock<Remote>`（`utils-core::SharedLock<T>`）等のラッパが存在しない
- **AND** `Remote` の field を installer 側から直接読み書きする経路が存在しない

### Requirement: 外部制御は Sender と JoinHandle のみで行う

run task に対する外部制御は次の 2 種のみで行う SHALL。

- `Sender<RemoteEvent>`（adapter 内部 mpsc の送信側を installer が clone 保持）
- `JoinHandle<Result<(), RemotingError>>`（spawn の戻り値）

これら以外（直接 method 呼出、共有 state 経由）で run task の `Remote` に触れてはならない（MUST NOT）。

#### Scenario: installer の保持 field

- **WHEN** `RemotingExtensionInstaller` または同等型の field を検査する
- **THEN** `event_sender: tokio::sync::mpsc::Sender<RemoteEvent>` / `run_handle: JoinHandle<Result<(), RemotingError>>` / `cached_addresses: Vec<Address>` 程度のみを保持する
- **AND** `Remote` への直接参照（`Arc<Remote>` / `&Remote`）を保持しない

#### Scenario: addresses クエリのキャッシュ経路

- **WHEN** `Remoting::addresses()` が呼ばれる
- **THEN** installer の `cached_addresses` を返す
- **AND** run 中の `Remote` インスタンスにアクセスしない

#### Scenario: addresses キャッシュの初期化

- **WHEN** installer が `Remote` を構築した直後に advertised addresses を取得する
- **THEN** `transport.start()` で listening を確立した後、`Remote::addresses()`（既存 inherent method）の戻り値を `Vec<Address>` として `cached_addresses` field に保存してから `Remote::run` を spawn する
- **AND** 取得経路は `Remote::addresses()` 一本に集約される（`Remote::start` 等の別 API は新設しない）
- **AND** 起動後にキャッシュは変更されない（addresses が変わる場合は本 change のスコープ外、別 change で扱う）

### Requirement: Remoting::shutdown の停止プロトコル

`Remoting::shutdown` は adapter 内部 sender 経由で `RemoteEvent::TransportShutdown` を push し、`JoinHandle` を await して run task の終了を待つ SHALL。

#### Scenario: shutdown の手順

- **WHEN** `Remoting::shutdown` が呼ばれる
- **THEN** installer は次の順序で処理する
  1. `event_sender.send(RemoteEvent::TransportShutdown).await`（戻り値の `Result` を `?` で伝播）
  2. `run_handle.await`（`JoinHandle` の戻り値を観測）
  3. JoinHandle 結果が `Ok(Ok(()))` であれば正常終了
- **AND** いずれかのステップが失敗した場合、`RemotingError` に変換して呼出元に伝播する

#### Scenario: Remote::run の異常終了の観測

- **WHEN** `Remote::run` が `Err(RemotingError::TransportUnavailable)` 等を返した
- **THEN** installer は `JoinHandle::await` の戻り値で error を log に記録し、actor system の error path に伝播する
- **AND** `let _ = ...` による無言握りつぶしは存在しない

#### Scenario: 別 Driver 型の不在

- **WHEN** adapter 側の installer 実装を検査する
- **THEN** `RemoteDriverHandle` や `RemoteDriverOutcome` を import / 利用していない（純増ゼロ方針）

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

- **WHEN** `Remote::run` が `AssociationEffect::StartHandshake { authority, timeout, generation }` を実行し、`RemoteTransport::send_handshake` の後に `RemoteTransport::schedule_handshake_timeout(&authority, timeout, generation)` を呼ぶ
- **THEN** adapter は `tokio::spawn(async move { tokio::time::sleep(timeout).await; sender.send(RemoteEvent::HandshakeTimerFired { authority, generation }).await; })` 相当の経路で timer を確保する
- **AND** generation 値は呼出引数から受け取った値をそのまま timer task が保持する
- **AND** 戻り値の `Result<(), TransportError>` は spawn 成功で `Ok(())` を返す（sleep の満了は非同期に行われるため、本メソッドは sleep を await しない）

#### Scenario: Timer trait の不在

- **WHEN** `modules/remote-core/src/core/` 配下のソースを検査する
- **THEN** `pub trait Timer` や `pub struct TimerToken` が定義されていない（純増ゼロ方針）

#### Scenario: 古い timer の発火許容

- **WHEN** Association の状態が次の Handshaking に進み、generation が +1 された後に古い timer が満了する
- **THEN** adapter は generation 値を変更せずに `HandshakeTimerFired { generation: g_event }` を push する
- **AND** `Remote::run` 側で「current generation `!=` g_event なので破棄」する判定が行われる（adapter 側でのキャンセルは不要）
- **AND** 判定は `!=` 比較で行い、大小比較（`>` / `<`）は使わない（`wrapping_add` の wrap 時にも stale 判定が漏れないようにするため）

### Requirement: outbound enqueue は RemoteEvent::OutboundEnqueued として通知する

local actor が remote ref に tell する経路で、adapter は `OutboundEnvelope` を構築した後 `AssociationRegistry` を直接 mutate せず、`RemoteEvent::OutboundEnqueued { authority, envelope }` を adapter 内部 sender 経由で receiver に push する SHALL。`Remote::run` がこの event を受けて `Association::enqueue` と outbound drain を実行する（実行責務は `remote-core-extension` capability で要件化済）。

#### Scenario: enqueue 経路

- **WHEN** local actor の tell から adapter 側 RemoteActorRef 相当に到達する
- **THEN** adapter は `OutboundEnvelope` を構築する
- **AND** `RemoteEvent::OutboundEnqueued { authority, envelope }` を adapter 内部 sender 経由で push する
- **AND** `Result` を `?` または `match` で扱う（`let _ = ...` での無言握りつぶしは禁止）

#### Scenario: AssociationRegistry 直接操作の不在

- **WHEN** adapter 側の RemoteActorRef / dispatch 経路を検査する
- **THEN** `AssociationRegistry::*` の状態変更メソッド（`enqueue` / `next_outbound` 等）を直接呼ぶ箇所が存在しない
- **AND** `Association` の `&mut` アクセスは `Remote::run` 経由でのみ行われる

#### Scenario: bounded channel での enqueue 失敗

- **WHEN** adapter 内部 sender が bounded で、buffer 満杯時に `try_send` が失敗する
- **THEN** adapter は失敗を transport-level error として扱い、必要に応じて caller に伝播するか、metrics として記録する
- **AND** sender 飢餓時の挙動は実装 PR で確定する（本 change のスコープ外、design.md の Open Questions 参照）

### Requirement: 効果適用から StartHandshake 分岐を削除する

adapter 側の `effect_application::apply_effects_in_place`（または相当箇所）から `AssociationEffect::StartHandshake` の dispatch 分岐を削除する SHALL。`Remote::run` 側が `RemoteTransport` 経由で handshake を開始するため、adapter 側で同 effect を扱う必要がない。

#### Scenario: StartHandshake 分岐の不在

- **WHEN** `modules/remote-adaptor-std/src/std/effect_application.rs` を検査する
- **THEN** `AssociationEffect::StartHandshake { .. } =>` 分岐が存在しない
- **AND** unreachable! の使用も存在しない（`Remote::run` がすべて処理するため adapter のこの経路を通らない）

#### Scenario: 残存 effect の adapter 処理

- **WHEN** adapter 側で扱うべき effect 種別を検査する
- **THEN** `SendEnvelopes`、`DiscardEnvelopes`、`PublishLifecycle` 等の I/O 直結 effect のみが adapter 側で扱われ、状態遷移を伴う effect は `Remote::run` 側に集約される
