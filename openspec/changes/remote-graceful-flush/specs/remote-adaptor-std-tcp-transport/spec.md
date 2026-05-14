## ADDED Requirements

### Requirement: TCP transport sends flush requests on targeted writer lanes

std TCP adaptor は core association が返す flush request effect を、対象 writer lane ごとの `ControlPdu::FlushRequest` frame として enqueue する SHALL。各 lane の flush request は、その lane に既に enqueue 済みの frame の後ろに置かれなければならない（MUST）。現行 TCP adaptor の writer lane は envelope を運び得る message-capable lane であり、`lane_id = 0` を control-only lane と仮定してはならない（MUST NOT）。

#### Scenario: shutdown flush targets supplied writer lanes

- **WHEN** core association が scope `Shutdown` の flush request effect を返す
- **THEN** TCP transport は effect に含まれる各 writer lane id に `ControlPdu::FlushRequest` を enqueue する
- **AND** 各 request は同じ flush id と expected ack 数を持つ
- **AND** lane ごとに異なる lane id を持つ

#### Scenario: DeathWatch flush targets all message-capable writer lanes

- **WHEN** core association が scope `BeforeDeathWatchNotification` の flush request effect を返す
- **THEN** TCP transport は effect に含まれる message-capable writer lane id すべてに `ControlPdu::FlushRequest` を enqueue する
- **AND** 現行 TCP adaptor では lane `0` も envelope を運び得るため、lane `0` を自動除外しない

#### Scenario: dedicated control-only lane is optional and excluded from DeathWatch scope

- **GIVEN** 将来の TCP 実装が envelope を運ばない dedicated control-only lane を追加している
- **WHEN** scope `BeforeDeathWatchNotification` の対象 lane set を構築する
- **THEN** dedicated control-only lane は対象から外してよい
- **AND** envelope を運び得る writer lane はすべて対象に含める

#### Scenario: lane backpressure is observable

- **GIVEN** flush request を enqueue すべき対象 lane の queue が full である
- **WHEN** TCP transport が flush request effect を実行する
- **THEN** failure は log または returned error path で観測できる
- **AND** association flush session は timeout または failure outcome へ進められる

### Requirement: TCP inbound dispatch routes flush control frames to core

std TCP adaptor は inbound `ControlPdu::FlushRequest` / `ControlPdu::FlushAck` を actor-core delivery へ渡さず、`RemoteEvent::InboundFrameReceived` または同等の core event として `Remote::handle_remote_event` へ渡す SHALL。flush request への ack 生成は、現行の heartbeat response と同じく core control-PDU handling が `RemoteTransport` 経由で行う。

#### Scenario: inbound flush request is routed as control frame

- **WHEN** TCP inbound lane が `ControlPdu::FlushRequest { flush_id, lane_id, expected_acks, .. }` を受信する
- **THEN** adaptor は `WireFrame::Control(ControlPdu::FlushRequest { .. })` を `RemoteEvent::InboundFrameReceived` として core event loop へ渡す
- **AND** flush request を actor-core envelope delivery へ進めない

#### Scenario: inbound flush ack is routed to core

- **WHEN** TCP inbound lane が `ControlPdu::FlushAck` を受信する
- **THEN** adaptor は `RemoteEvent::InboundFrameReceived` または同等の core event として ack を `Remote::handle_remote_event` へ渡す
- **AND** core association が flush session state を更新できる

#### Scenario: flush ack send failure is observable

- **WHEN** core が inbound flush request に対する flush ack を `RemoteTransport` 経由で送信し、transport が失敗を返す
- **THEN** failure は log または returned error path に残す
- **AND** failure を silent drop しない
