# actor-std-adapter-surface Specification

## Purpose
`modules/actor-adaptor/src/std` の公開面を adapter と std 固有 helper のみに制限し、core 横流し façade を排除する。

## Requirements
### Requirement: `std` 公開面は adapter と std 固有 helper のみに限定される
`modules/actor-adaptor/src/std` は、`core` の port を std/tokio/tracing に接続する adapter 実装、または std 固有 helper のみを公開しなければならない。`core` 型を包み直しただけの façade / wrapper は外部公開してはならず、公開面に現れてはならない。classic logging family、event stream shim、core 横流し helper は `std` 公開面に残存してはならず、公開面は adapter と std 固有 helper のみに MUST 制限される。

#### Scenario: classic logging facade が `std` 公開面から除外される
- **WHEN** 利用者が `fraktor_actor_adaptor_rs::std::event::logging` を参照する
- **THEN** `ActorLogMarker`、`ActorLogging`、`DiagnosticActorLogging`、`LoggingReceive`、`NoLogging`、`LoggingAdapter`、`BusLogging` は公開されない
- **AND** `TracingLoggerSubscriber` のような runtime adapter だけが live entry point として残る

#### Scenario: event stream shim が `std` 公開面から除外される
- **WHEN** 利用者が `fraktor_actor_adaptor_rs::std::event::stream` を参照する
- **THEN** `EventStreamSubscriberShared` や `subscriber_handle` のような `core` 横流し shim は公開されない
- **AND** `std::event::stream` に残る public 型は `DeadLetterLogSubscriber` のみである

#### Scenario: pattern wrapper が `std` 公開面から除外される
- **WHEN** 利用者が `fraktor_actor_adaptor_rs::std::pattern` を参照する
- **THEN** `ask_with_timeout`、`graceful_stop`、`graceful_stop_with_message`、`retry` のような core 横流し helper は公開されない
- **AND** `std::pattern` に残る public API は `StdClock`、`CircuitBreaker`、`CircuitBreakerShared`、`circuit_breaker`、`circuit_breaker_shared` のみである

### Requirement: examples と tests は pure wrapper ではなく core API に依存する
`showcases/std/*`、`modules/actor-adaptor/src/std/tests.rs`、`modules/actor-adaptor/src/std/pattern/tests.rs`、および削除対象 wrapper に依存している内部コードは、pure wrapper ではなく `core` API に依存しなければならない。この依存は `std` façade ではなく `core` API へ MUST 向かなければならない。

#### Scenario: std showcase が core logging API を使う
- **WHEN** `showcases/std/classic_logging/main.rs` を確認する
- **THEN** classic logging capability は `core` 側の型と API を使って表現される
- **AND** `fraktor_actor_adaptor_rs::std::event::logging` の facade には依存しない

#### Scenario: event stream 利用コードが core subscriber 型を使う
- **WHEN** showcase または `modules/actor-adaptor/src/std/tests.rs` が event stream subscriber を使う
- **THEN** `core::kernel::event::stream::EventStreamSubscriberShared` と `subscriber_handle` を使う
- **AND** `std::event::stream` の shim を前提としない

#### Scenario: std pattern tests は残存 API だけを前提にする
- **WHEN** `modules/actor-adaptor/src/std/pattern/tests.rs` を確認する
- **THEN** test は `StdClock`、`CircuitBreaker`、`CircuitBreakerShared`、`circuit_breaker`、`circuit_breaker_shared` だけを参照する
- **AND** 削除対象の `ask_with_timeout`、`graceful_stop`、`graceful_stop_with_message`、`retry` を前提にしない
