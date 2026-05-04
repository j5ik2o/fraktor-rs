## ADDED Requirements

### Requirement: schedule_handshake_timeout メソッド

`RemoteTransport` trait は handshake request を送信する method と handshake timeout を adapter 側に予約させる method を持たなければならない（MUST）。`Remote::handle_remote_event` が `AssociationEffect::StartHandshake { authority, timeout, generation }` を実行する際、handshake request frame の送信（`send_handshake`）に続けて timeout 予約 method を呼び、adapter 側 timer task を起動する。

#### Scenario: send_handshake メソッドのシグネチャ

- **WHEN** `modules/remote-core/src/core/transport/remote_transport.rs` の `RemoteTransport` trait 定義を読む
- **THEN** `fn send_handshake(&mut self, remote: &Address, pdu: HandshakePdu) -> Result<(), TransportError>` または同等のシグネチャが宣言されている
- **AND** メソッドは同期 `&mut self` で、`async fn` ではない（既存 trait 契約と整合）

#### Scenario: メソッドのシグネチャ

- **WHEN** `modules/remote-core/src/core/transport/remote_transport.rs` の `RemoteTransport` trait 定義を読む
- **THEN** `fn schedule_handshake_timeout(&mut self, authority: &TransportEndpoint, timeout: core::time::Duration, generation: u64) -> Result<(), TransportError>` または同等のシグネチャが宣言されている
- **AND** メソッドは同期 `&mut self` で、`async fn` ではない（既存 trait 契約と整合）

#### Scenario: 引数のセマンティクス

- **WHEN** `schedule_handshake_timeout` の rustdoc を読む
- **THEN** `authority` は handshake 対象の remote authority、`timeout` は満了までの時間、`generation` は `Association` が当該 handshake 試行に発行した単調増加 `u64` 値であることが明記されている
- **AND** adapter 実装は満了時に `RemoteEvent::HandshakeTimerFired { authority, generation }` を adapter 内部 sender 経由で receiver に push する責務を持つ

#### Scenario: 同 authority への重複呼出

- **WHEN** 同じ `authority` に対して `schedule_handshake_timeout` を 2 回連続で呼ぶ
- **THEN** 2 回目の呼出は前回の timer task をキャンセルせず、独立した task として動作してよい
- **AND** 古い timer が満了して push する `HandshakeTimerFired` は `generation` 値が古いため `Remote::handle_remote_event` 側で `!=` 判定により破棄される（adapter 側でキャンセル責務を負わない）

#### Scenario: 戻り値の握りつぶし禁止

- **WHEN** `Remote::handle_remote_event` が `schedule_handshake_timeout` を呼ぶ実装を検査する
- **THEN** 戻り値の `Result<(), TransportError>` は `?` で伝播するか `match` で観測する
- **AND** `let _ = ...` で握りつぶしている経路が存在しない

### Requirement: handshake 関連 method 以外の timer 予約 API を本 change で追加しない

quarantine timer / large message ack timer 等、`schedule_handshake_timeout` 以外の遅延発火 API を `RemoteTransport` に追加してはならない（MUST NOT）。これらが必要になった時点で別 change として scheduling 経路を確定する。

#### Scenario: 他 timer 系 method の不在

- **WHEN** `RemoteTransport` trait のメソッド一覧を検査する
- **THEN** `schedule_quarantine_timer` / `schedule_drain_timer` / `schedule_event` など、handshake 以外の timer 予約 method が宣言されていない
- **AND** Timer Port 相当の汎用 trait（`pub trait Timer` 等）も存在しない

#### Scenario: スケジューリング経路の限定理由（spec 内 rationale）

- **WHEN** scheduling 系 API を一般化したい衝動が生じる
- **THEN** 本 change のスコープが「handshake timer の adapter 責務化」であり、QuarantineTimer / OutboundFrameAcked / BackpressureCleared 等の event は scheduling 経路と一緒に別 change で追加する方針であることを根拠に却下する
- **AND** YAGNI 原則と純増ゼロ方針を維持する
