# dispatch-executor-unification Specification

## Purpose
TBD - created by archiving change actor-core-std-separation-improvement. Update Purpose after archive.
## Requirements
### Requirement: TokioExecutor が core::DispatchExecutor を直接実装する

`std::dispatch::dispatcher::dispatch_executor::TokioExecutor` が `core::dispatch::dispatcher::DispatchExecutor` trait を直接実装する。中間の `std::dispatch::dispatcher::DispatchExecutor` trait を経由しない。

#### Scenario: TokioExecutor が core trait を実装する
- **WHEN** `TokioExecutor` を `Box<dyn core::dispatch::dispatcher::DispatchExecutor>` にキャストする
- **THEN** コンパイルが成功する

#### Scenario: TokioExecutor の execute が dispatcher を駆動する
- **WHEN** `TokioExecutor::execute()` に `DispatchShared` を渡す
- **THEN** `tokio::runtime::Handle::spawn_blocking` で dispatcher が駆動される

### Requirement: ThreadedExecutor が core::DispatchExecutor を直接実装する

`std::dispatch::dispatcher::dispatch_executor::ThreadedExecutor` が `core::dispatch::dispatcher::DispatchExecutor` trait を直接実装する。

#### Scenario: ThreadedExecutor が core trait を実装する
- **WHEN** `ThreadedExecutor` を `Box<dyn core::dispatch::dispatcher::DispatchExecutor>` にキャストする
- **THEN** コンパイルが成功する

#### Scenario: ThreadedExecutor の execute が OS スレッドを生成する
- **WHEN** `ThreadedExecutor::execute()` に `DispatchShared` を渡す
- **THEN** `std::thread::Builder::spawn` で新規スレッド上で dispatcher が駆動される

