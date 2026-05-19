## ADDED Requirements

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

## REMOVED Requirements

### Requirement: std::dispatch::dispatcher::DispatchExecutor trait
**Reason**: core::dispatch::dispatcher::DispatchExecutor に統一。TokioExecutor / ThreadedExecutor は両方 Sync を満たすため、core trait を直接実装可能。
**Migration**: `use crate::std::dispatch::dispatcher::DispatchExecutor` → `use crate::core::dispatch::dispatcher::DispatchExecutor`

### Requirement: DispatchExecutorAdapter ブリッジ型
**Reason**: std 版 DispatchExecutor trait の廃止により不要。core trait への直接実装で adapter の役割が消滅。
**Migration**: `DispatchExecutorAdapter::new(executor)` → executor を直接 `Box<dyn core::DispatchExecutor>` として使用

### Requirement: DispatcherConfig が StdSyncMutex ラップ済み executor を受け取る
**Reason**: adapter 層の廃止に伴い、executor の受け取り方を core trait ベースに統一。
**Migration**: `DispatcherConfig::from_executor(ArcShared<StdSyncMutex<Box<dyn std::DispatchExecutor>>>)` → `DispatcherConfig::from_executor(Box<dyn core::DispatchExecutor>)`
