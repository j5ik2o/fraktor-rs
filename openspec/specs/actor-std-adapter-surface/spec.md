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

### Requirement: std adapter 公開面は actor runtime の shared factory / lock factory override surface を公開してはならない

std adapter の system 公開面は、actor runtime の shared wrapper / shared state backend を差し替える surface を公開してはならない（MUST NOT）。`shared_factory` module、`StdActorSharedFactory`、`DebugActorSharedFactory`、およびそれらの rename 版となる lock factory concrete 型を公開面に残してはならない（MUST NOT）。

#### Scenario: std 公開面から shared factory module が除外される
- **WHEN** 利用者が std adapter の system 公開面を確認する
- **THEN** `std::system::shared_factory` module は存在しない
- **AND** `StdActorSharedFactory` と `DebugActorSharedFactory` は利用できない
- **AND** `StdActorLockFactory` や `DebugActorLockFactory` のような代替公開型も存在しない

#### Scenario: std adapter 利用コードは default builtin spin 構成を前提にする
- **WHEN** std adapter を使う example、test、または利用コードが actor system を構築する
- **THEN** それらは `with_shared_factory(...)` や `with_lock_factory(...)` を使わない
- **AND** actor runtime の shared wrapper / shared state backend 切替を前提にしない

### Requirement: std adapter は termination blocking 用の `Blocker` 実装を提供しなければならない

std adapter は、core の `Blocker` port 契約を満たす termination 用 blocking 実装を提供しなければならない。これにより同期 std アプリケーションは busy wait なしで actor system termination を待機できなければならない。

#### Scenario: std adapter から `Blocker` 実装を取得できる
- **WHEN** 利用者が std 環境で actor system termination を同期的に待ちたい
- **THEN** `fraktor_actor_adaptor_rs::std` 配下から `Blocker` 契約を満たす型または helper に到達できる
- **AND** caller は `thread::yield_now()` ループを自前で書かなくてよい

#### Scenario: std adapter の blocking 実装は core の termination 契約と整合する
- **WHEN** std adapter の `Blocker` 実装を使って `TerminationSignal` の完了を待つ
- **THEN** actor system termination 後に待機は解除される
- **AND** 複数 observer が同じ `TerminationSignal` を観測しても終了状態が消費されない

