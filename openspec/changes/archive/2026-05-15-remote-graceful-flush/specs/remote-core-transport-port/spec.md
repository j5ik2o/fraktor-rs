## ADDED Requirements

### Requirement: RemoteTransport supports lane-targeted flush request delivery

`RemoteTransport` は flush request を指定された outbound writer lane へ bounded に enqueue する command を提供する SHALL。この command は `RemoteTransport` の既存同期 port 境界に従い、`RemoteShared` / `Remote` へ再入してはならず（MUST NOT）、async wait、timer sleep、actor-core delivery、`JoinHandle` 待機を行ってはならない（MUST NOT）。

#### Scenario: flush request is sent to a specific writer lane

- **WHEN** `Remote` が `ControlPdu::FlushRequest { lane_id: 2, .. }` を remote transport へ渡す
- **THEN** transport は remote peer の writer lane `2` に flush request frame を enqueue する
- **AND** lane `2` に既に enqueue 済みの frame より前に flush request を挿入しない

#### Scenario: lane backpressure is reported

- **GIVEN** target writer lane の bounded queue が full である
- **WHEN** `Remote` が flush request をその lane へ送ろうとする
- **THEN** transport は `TransportError::Backpressure` または同等の observable error を返す
- **AND** error を silent drop しない

#### Scenario: flush timer scheduling is not added to RemoteTransport

- **WHEN** `RemoteTransport` の flush 関連 API を検査する
- **THEN** flush timeout の sleep / scheduling method は追加されない
- **AND** flush timeout は std adaptor の timer task が `RemoteEvent` または同等の input として core へ戻す
