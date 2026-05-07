## MODIFIED Requirements

### Requirement: Remoting trait の共有 surface

`Remoting` trait のすべてのメソッドは `&self` を取る同期 method SHALL。`async fn` および `Future` 戻り値を追加してはならない（MUST NOT）。`addresses` の戻り値は owned `Vec<Address>`。`impl Remoting for Remote` は存在してはならず、共有 surface である `RemoteShared` が `Remoting` を実装する SHALL。

#### Scenario: pivot 前の古い signature は存在しない

- **WHEN** `openspec/specs/remote-core-extension/spec.md` と `modules/remote-core/src/core/extension/remoting.rs` を検査する
- **THEN** `start(&mut self)` / `shutdown(&mut self)` / `addresses(&self) -> &[Address]` を要求する scenario は存在しない
- **AND** 実装は `fn start(&self)`, `fn shutdown(&self)`, `fn quarantine(&self, ...)`, `fn addresses(&self) -> Vec<Address>` を持つ

### Requirement: RemoteShared の event-step orchestration

adapter が inbound delivery hook を必要とする場合、`RemoteShared` は 1 件の `RemoteEvent` を core logic に委譲して処理する最小 orchestration API を提供してよい（MAY）。この API は raw lock を露出してはならず（MUST NOT）、event semantics を `RemoteShared` に重複実装してはならない（MUST NOT）。

#### Scenario: raw SharedLock を露出しない event-step API

- **WHEN** `RemoteShared` に event-step API を追加する
- **THEN** 戻り値や引数に `SharedLock<Remote>` / lock guard 型は現れない
- **AND** 実装は内部で `Remote::handle_remote_event(event)` と `Remote::should_stop_event_loop()` または同等 Query に委譲する
- **AND** `RemoteShared` 自身は `match event` で association / transport semantics を実装しない

### Requirement: outbound watermark backpressure の意味

`Remote::handle_remote_event` は outbound enqueue / dequeue のたびに `Association::total_outbound_len()` を `RemoteConfig::outbound_high_watermark` / `outbound_low_watermark` と比較し、watermark 境界をエッジで跨いだ時にのみ backpressure signal を発火する SHALL。

#### Scenario: high watermark は internal drain を止めない

- **WHEN** high watermark 超過を検出する
- **THEN** 実装は internal drain が同じ user queue を dequeue できる状態を保つ
- **AND** `BackpressureSignal::Notify` を採用する場合は `record_backpressure(.., Notify, ..)` で観測可能にする
- **AND** `BackpressureSignal::Apply` を採用する場合は drain helper が user queue pause によって停止しないことを test で証明する

#### Scenario: live spec と実装が同じ signal を要求する

- **WHEN** `openspec/specs/remote-core-extension/spec.md`, `openspec/specs/remote-core-association-state-machine/spec.md`, `modules/remote-core/src/core/extension/remote.rs` を比較する
- **THEN** high watermark で使う signal が一致している
- **AND** `Notify` が public enum に残る場合、spec は `Apply` / `Release` のみと主張しない
