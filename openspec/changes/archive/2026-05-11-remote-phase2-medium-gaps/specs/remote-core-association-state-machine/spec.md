## MODIFIED Requirements

### Requirement: SendQueue priority logic

`Association` 内の `SendQueue` は system priority、normal user、large-message user の queue を持ち、system priority を最優先で取り出す SHALL。normal user と large-message user はどちらも wire 上は user message であり、large-message queue は送信側の local scheduling と capacity 分離のために使われる。

`Association::next_outbound` と `Association::apply_backpressure` は内部の `SendQueue` に委譲することで、この priority / lane ロジックを公開する。

#### Scenario: system queue の優先

- **WHEN** system priority、normal user、large-message user のすべてに message があり、`Association::next_outbound()` を呼ぶ
- **THEN** system priority の message が先に返される

#### Scenario: normal user は large-message user より先に drain される

- **WHEN** normal user と large-message user の両方の message が queue にあり、system priority message がない
- **THEN** normal user message が large-message user message より先に返される

#### Scenario: user queue の backpressure pause

- **WHEN** `Association` に `apply_backpressure(BackpressureSignal::Apply)` を適用してから `next_outbound()` を呼ぶ
- **THEN** normal user と large-message user の message は取り出されない
- **AND** system priority message は取り出される

#### Scenario: backpressure release

- **WHEN** `apply_backpressure(BackpressureSignal::Release)` を適用してから `next_outbound()` を呼ぶ
- **THEN** normal user と large-message user の message も取り出される

### Requirement: Association は large-message destination settings を enqueue に反映する

`Association::from_config` は `RemoteConfig::large_message_destinations()` と `RemoteConfig::outbound_large_message_queue_size()` を使って large-message enqueue policy を構成しなければならない (MUST)。

`Association::enqueue` は `OutboundPriority::User` の envelope について recipient absolute path が configured large-message destination pattern に一致する場合、normal user queue ではなく large-message queue に offer しなければならない (MUST)。`OutboundPriority::System` の envelope は pattern に一致しても system queue に入らなければならない (MUST)。

#### Scenario: matching user recipient は large-message queue に入る

- **GIVEN** `RemoteConfig` に `/user/large-*` の large-message destination pattern が設定されている
- **AND** `Association` がその config から作られている
- **WHEN** recipient path `/user/large-worker` の user envelope を enqueue する
- **THEN** envelope は large-message queue に入る
- **AND** normal user queue capacity は消費しない

#### Scenario: non-matching user recipient は normal user queue に入る

- **GIVEN** `RemoteConfig` に `/user/large-*` の large-message destination pattern が設定されている
- **WHEN** recipient path `/user/small-worker` の user envelope を enqueue する
- **THEN** envelope は normal user queue に入る
- **AND** large-message queue capacity は消費しない

#### Scenario: system envelope は large-message pattern より優先される

- **GIVEN** large-message destination pattern に一致する recipient path を持つ system envelope
- **WHEN** `Association::enqueue` を呼ぶ
- **THEN** envelope は system queue に入る
- **AND** large-message queue capacity は消費しない

#### Scenario: large-message queue capacity は config から来る

- **GIVEN** `RemoteConfig::with_outbound_large_message_queue_size(1)` で作られた `Association`
- **WHEN** matching user envelope を 2 件 enqueue する
- **THEN** 1 件目は accepted になる
- **AND** 2 件目は元 envelope を保持した queue-full outcome になる
