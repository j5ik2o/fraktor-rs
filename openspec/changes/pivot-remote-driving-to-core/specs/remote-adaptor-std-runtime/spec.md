## REMOVED Requirements

### Requirement: outbound loop (送信 tokio task)

**理由:** `Remote::run` が core 側で outbound 駆動を担当するため、adapter 側の `outbound_loop` は不要となる。`Remote::run` が `Association::next_outbound()` を呼び、`Codec::encode` を経て `RemoteTransport` に送信する経路に置き換えられる。

### Requirement: handshake driver (タイムアウト監視)

**理由:** handshake timeout は `AssociationEffect::StartHandshake { authority, timeout, generation }` を実行する際に adapter 側 I/O ワーカーが per-association tokio task として確保し、満了時に `RemoteEvent::HandshakeTimerFired { authority, generation }` を adapter 内部 sender 経由で source に push する形に置き換えられるため、`handshake_driver.rs` 単体としての独立 task ファイルは不要となる。

## MODIFIED Requirements

### Requirement: inbound dispatch (受信 I/O ワーカー)

adapter 側に受信 tokio task が定義され、TCP から受信した frame を `RemoteEvent::InboundFrameReceived { authority, frame }` として adapter 内部 sender 経由で `RemoteEventSource` に push する SHALL。`Association` の `handshake_accepted` 等を **直接呼んではならない**（MUST NOT）— state machine への反映は core 側の `Remote::run` が担当する。

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

### Requirement: Remote::run を tokio task として spawn する起動経路

`RemotingExtensionInstaller` または同等の installer は `Remote::run` を tokio task として spawn し、`JoinHandle` を保持する SHALL。actor system の停止時に `Remoting::shutdown` を呼び、adapter 内部 sender 経由で `RemoteEvent::TransportShutdown` を push して `Remote::run` task の終了を待つ。

#### Scenario: Remote::run の spawn

- **WHEN** `RemotingExtensionInstaller` が remote 拡張を actor system に登録する
- **THEN** 内部で `tokio::spawn` または同等の経路で `remote.run(&mut source)` を起動する
- **AND** 起動結果として `JoinHandle<Result<(), RemotingError>>` を保持する

#### Scenario: Remote::run の正常停止

- **WHEN** actor system が停止フェーズに入り、`Remoting::shutdown` が呼ばれる
- **THEN** installer は adapter 内部 sender に `RemoteEvent::TransportShutdown` を push する
- **AND** `Remote::run` task の `JoinHandle::await` を待ち、`Ok(())` を確認する

#### Scenario: Remote::run の異常終了の観測

- **WHEN** `Remote::run` が `Err(RemotingError::TransportUnavailable)` 等を返した
- **THEN** installer は error を log に記録し、actor system の error path に伝播する
- **AND** `let _ = ...` による無言握りつぶしは存在しない

#### Scenario: 別 Driver 型の不在

- **WHEN** adapter 側の installer 実装を検査する
- **THEN** `RemoteDriverHandle` や `RemoteDriverOutcome` を import / 利用していない（純増ゼロ方針）

### Requirement: tokio mpsc ベースの RemoteEventSource

adapter 側に `tokio::sync::mpsc` 受信側ラッパとして `RemoteEventSource` 実装が定義される SHALL。送信側 `Sender` は adapter 内部の I/O ワーカー / handshake timer task が clone して共有し、`RemoteEventSink` 等の trait としては core に公開しない。

#### Scenario: Source 実装の存在

- **WHEN** `modules/remote-adaptor-std/src/std/tokio_remote_event_source.rs` を読む
- **THEN** `pub struct TokioMpscRemoteEventSource` または同等の型が定義され、`impl RemoteEventSource` を持つ
- **AND** 内部で `tokio::sync::mpsc::Receiver<RemoteEvent>` を保持する

#### Scenario: Sink trait の不在

- **WHEN** `modules/remote-core/src/core/extension/` 配下のソースを検査する
- **THEN** `pub trait RemoteEventSink` または同等の trait が定義されていない（adapter 内部の `Sender` は core に公開せず、純増ゼロ方針を維持する）

#### Scenario: bounded / unbounded の選択

- **WHEN** Source 実装の channel 形式を検査する
- **THEN** bounded（背圧あり）か unbounded（背圧なし）かが docstring に明記されている
- **AND** 既定では bounded を選択し、capacity は `RemoteConfig` から読む経路を持つ

### Requirement: handshake timer は adapter 内部 task として確保される

`AssociationEffect::StartHandshake { authority, timeout, generation }` の実行に伴う handshake timeout は、adapter 側 I/O ワーカーが per-association tokio task として `tokio::time::sleep` を起動し、満了時に `RemoteEvent::HandshakeTimerFired { authority, generation }` を adapter 内部 sender 経由で source に push する SHALL。core 側に Timer Port を新設してはならない（MUST NOT）。

#### Scenario: handshake timer の発火経路

- **WHEN** `Remote::run` が `AssociationEffect::StartHandshake { authority, timeout, generation }` を実行する
- **THEN** adapter は `tokio::spawn(async move { tokio::time::sleep(timeout).await; sender.send(RemoteEvent::HandshakeTimerFired { authority, generation }).await; })` 相当の経路で timer を確保する
- **AND** generation 値は effect から受け取った値をそのまま timer task が保持する

#### Scenario: Timer trait の不在

- **WHEN** `modules/remote-core/src/core/` 配下のソースを検査する
- **THEN** `pub trait Timer` や `pub struct TimerToken` が定義されていない（純増ゼロ方針）

#### Scenario: 古い timer の発火許容

- **WHEN** Association の状態が次の Handshaking に進み、generation が +1 された後に古い timer が満了する
- **THEN** adapter は generation 値を変更せずに `HandshakeTimerFired { generation: g_old }` を push する
- **AND** `Remote::run` 側で「current generation > g_old なので破棄」する判定が行われる（adapter 側でのキャンセルは不要）

### Requirement: 効果適用から StartHandshake 分岐を削除する

adapter 側の `effect_application::apply_effects_in_place`（または相当箇所）から `AssociationEffect::StartHandshake` の dispatch 分岐を削除する SHALL。`Remote::run` 側が `RemoteTransport` 経由で handshake を開始するため、adapter 側で同 effect を扱う必要がない。

#### Scenario: StartHandshake 分岐の不在

- **WHEN** `modules/remote-adaptor-std/src/std/effect_application.rs` を検査する
- **THEN** `AssociationEffect::StartHandshake { .. } =>` 分岐が存在しない
- **AND** unreachable! の使用も存在しない（`Remote::run` がすべて処理するため adapter のこの経路を通らない）

#### Scenario: 残存 effect の adapter 処理

- **WHEN** adapter 側で扱うべき effect 種別を検査する
- **THEN** `SendEnvelopes`、`DiscardEnvelopes`、`PublishLifecycle` 等の I/O 直結 effect のみが adapter 側で扱われ、状態遷移を伴う effect は `Remote::run` 側に集約される
