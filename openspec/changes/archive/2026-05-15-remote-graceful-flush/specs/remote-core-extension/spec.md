## ADDED Requirements

### Requirement: Remote exposes flush start, ack, timer, and outcome surface

`Remote` / `RemoteShared` は active association の flush 開始、inbound `FlushAck`、flush timer input、connection loss による flush release を扱う SHALL。std adaptor が shutdown waiter と DeathWatch pending notification を解放できるよう、flush completed / timed-out / failed outcome を `RemoteShared` の event-step return value、drain API、または同等の lock-free-after-step surface で観測可能にしなければならない（MUST）。

`RemoteShared` は raw lock guard や `Association` 参照を std adaptor へ公開してはならない（MUST NOT）。std adaptor は `Association` を直接操作せず、`RemoteShared` の最小 API または `RemoteEvent` 経由で flush input を渡す。

#### Scenario: shutdown flush starts before shutdown transition

- **WHEN** std adaptor が shutdown flush を開始する
- **THEN** `Remote` は active association ごとに flush target writer lane set を受け取る
- **AND** 対象 association の prior outbound queue を drain してから flush request effect を実行する
- **AND** drain できない場合は flush start failure または timeout outcome を観測可能にする
- **AND** `RemoteShared::shutdown` は flush wait の後に呼ばれる

#### Scenario: inbound FlushAck updates association and exposes outcome

- **WHEN** `RemoteEvent::InboundFrameReceived` または同等の input で `ControlPdu::FlushAck` を受信する
- **THEN** `Remote` は対象 association の flush session に ack を適用する
- **AND** session が完了した場合は flush completed outcome を std adaptor が write lock 外で観測できる

#### Scenario: inbound FlushRequest returns ack through transport

- **WHEN** `RemoteEvent::InboundFrameReceived` または同等の input で `ControlPdu::FlushRequest { flush_id, lane_id, expected_acks, .. }` を受信する
- **THEN** `Remote` は同じ flush id、lane id、expected ack 数を持つ `ControlPdu::FlushAck` を `RemoteTransport` 経由で送信元へ返す
- **AND** flush request を actor-core envelope delivery へ進めない

#### Scenario: flush timer input releases pending session

- **WHEN** std adaptor の timer task が flush deadline 到達を core に入力する
- **THEN** `Remote` は matching flush session を timed-out outcome として解放する
- **AND** stale flush id または既に完了済みの timer input は no-op として扱う

#### Scenario: connection loss releases pending flush

- **GIVEN** active association に pending flush session がある
- **WHEN** `RemoteEvent::ConnectionLost` または quarantine transition が association に適用される
- **THEN** `Remote` は pending flush session を failed または timed-out outcome として解放する
- **AND** std adaptor は shutdown waiter または DeathWatch pending notification を先へ進められる
