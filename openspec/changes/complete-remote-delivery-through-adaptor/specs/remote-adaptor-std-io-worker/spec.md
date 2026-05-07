## MODIFIED Requirements

### Requirement: inbound dispatch（受信 tokio task）

adapter 側に受信 tokio task が定義され、TCP から受信した core wire frame を `RemoteEvent::InboundFrameReceived { authority, frame, now_ms }` として adapter 内部 sender 経由で `RemoteEventReceiver` に push する SHALL。`Association` の状態遷移メソッドを直接呼んではならない（MUST NOT）。state machine への反映は core 側の `Remote::handle_remote_event` が担当する。

#### Scenario: 受信 frame の event push

- **WHEN** 受信 loop が TCP frame を受信する
- **THEN** adapter は decoded core wire frame と adapter の monotonic clock から `RemoteEvent::InboundFrameReceived { authority, frame, now_ms }` を構築する
- **AND** adapter 内部の sender（`tokio::sync::mpsc::Sender<RemoteEvent>` 等）に push し、`Result` を観測可能に扱う

#### Scenario: Association 直接呼び出しの不在

- **WHEN** `modules/remote-adaptor-std/src/std/association/inbound_dispatch.rs` または同等のソースを検査する
- **THEN** `Association::handshake_accepted` / `accept_handshake_request` / `accept_handshake_response` 等の core state 遷移メソッドを直接呼ぶ箇所が存在しない

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
