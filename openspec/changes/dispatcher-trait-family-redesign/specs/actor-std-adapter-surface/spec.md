## MODIFIED Requirements

### Requirement: std adapter の dispatcher 公開面は policy family と helper に限定される

`modules/actor-adaptor/src/std` における dispatcher 公開面は、policy family と std 固有 helper のみに限定されなければならない。config 型、executor 型、core façade wrapper を公開面に出してはならない。

#### Scenario: std adapter は policy family を公開する
- **WHEN** `fraktor_actor_adaptor_rs::std::dispatch::dispatcher` の公開面を確認する
- **THEN** `tokio-executor` feature 有効時は `DefaultDispatcher`、`PinnedDispatcher`、`BlockingDispatcher` の policy family が公開される
- **AND** `tokio-executor` feature 無効時に `DefaultDispatcher` を thread backend へ fallback して公開しない

#### Scenario: config / executor 型は std adapter の public policy surface に現れない
- **WHEN** `fraktor_actor_adaptor_rs::std::dispatch::dispatcher` の公開面を確認する
- **THEN** `DispatcherConfig`、`DispatchExecutor`、`DispatchExecutorRunner` は public policy surface に含まれない
- **AND** `TokioExecutor` や `ThreadedExecutor` は internal backend 実装としてのみ扱われる

#### Scenario: std adapter は core ActorSystem facade を再公開しない
- **WHEN** `fraktor_actor_adaptor_rs::std` の公開面を確認する
- **THEN** `core::ActorSystem` を包み直した façade / wrapper は存在しない
- **AND** showcase や cluster helper は core の `ActorSystem` に依存する

#### Scenario: std tick driver helper は leaf module から直接使われる
- **WHEN** std adapter の tick driver helper 公開面を確認する
- **THEN** `default_tick_driver_config` や `tick_driver_config_with_resolution` は leaf module の public API として提供される
- **AND** `std.rs` に中継用の横流し wrapper 関数は存在しない
