## 1. CircuitBreaker の core 移設（Capability A）

- [ ] 1.1 `core/pattern/clock.rs` に `Clock` trait を定義する（`type Instant`, `now()`, `elapsed_since()`）
- [ ] 1.2 `std/pattern/std_clock.rs` に `StdClock` 構造体を作成し `Clock` trait を実装する（`std::time::Instant` ベース）
- [ ] 1.3 `std/pattern/circuit_breaker_state.rs` を `core/pattern/circuit_breaker_state.rs` に移動する
- [ ] 1.4 `std/pattern/circuit_breaker_call_error.rs` を `core/pattern/circuit_breaker_call_error.rs` に移動する
- [ ] 1.5 `std/pattern/circuit_breaker_open_error.rs` を `core/pattern/circuit_breaker_open_error.rs` に移動する
- [ ] 1.6 `std/pattern/circuit_breaker.rs` を `core/pattern/circuit_breaker.rs` に移動し、`CircuitBreaker<C: Clock>` にジェネリック化する（`Box<dyn Fn() -> Instant>` を `Clock` trait に置き換え）
- [ ] 1.7 `std/pattern/circuit_breaker_shared.rs` を `core/pattern/circuit_breaker_shared.rs` に移動し、`CircuitBreakerShared<C: Clock>` にジェネリック化する
- [ ] 1.8 `core/pattern.rs` のモジュール宣言に新規モジュールを追加し、公開型を re-export する
- [ ] 1.9 `std/pattern.rs` を更新: core 型の re-export + `StdClock` を使った型エイリアス（`type CircuitBreaker = core::pattern::CircuitBreaker<StdClock>` 等）+ ファクトリ関数
- [ ] 1.10 `std/pattern/circuit_breaker/tests.rs` と `std/pattern/circuit_breaker_shared/tests.rs` を移動・更新し、テストが通ることを確認する
- [ ] 1.11 `core/pattern/` に Clock のテスト（FakeClock を使った CircuitBreaker のユニットテスト）を追加する

## 2. CoordinatedShutdown 型の core 移設（Capability D）

- [ ] 2.1 `std/system/coordinated_shutdown_error.rs` を `core/system/coordinated_shutdown_error.rs` に移動する
- [ ] 2.2 `std/system/coordinated_shutdown_id.rs` を `core/system/coordinated_shutdown_id.rs` に移動する
- [ ] 2.3 `std/system/coordinated_shutdown_phase.rs` を `core/system/coordinated_shutdown_phase.rs` に移動する
- [ ] 2.4 `std/system/coordinated_shutdown_reason.rs` を `core/system/coordinated_shutdown_reason.rs` に移動する
- [ ] 2.5 `core/system.rs` のモジュール宣言に追加し、公開型を re-export する
- [ ] 2.6 `std/system.rs` を更新: 移設した型を core から re-export し、`#[cfg(feature = "tokio-executor")]` gate を型定義から外す
- [ ] 2.7 `std/system/coordinated_shutdown.rs` と `coordinated_shutdown_installer.rs` の import パスを更新する
- [ ] 2.8 コンパイルとテストが通ることを確認する

## 3. DispatchExecutor の統合（Capability B）

- [ ] 3.1 `std/dispatch/dispatcher/dispatch_executor/tokio_executor.rs` を変更: `use crate::std::dispatch::dispatcher::DispatchExecutor` → `use crate::core::dispatch::dispatcher::DispatchExecutor` にして core trait を直接実装する
- [ ] 3.2 `std/dispatch/dispatcher/dispatch_executor/thread_executor.rs` を同様に変更する
- [ ] 3.3 `std/dispatch/dispatcher/dispatcher_config.rs` を変更: executor の受け取りを `Box<dyn core::DispatchExecutor>` に統一する（`ArcShared<StdSyncMutex<...>>` ラップを除去）
- [ ] 3.4 `std/dispatch/dispatcher/base.rs` の std 版 `DispatchExecutor` trait を削除する
- [ ] 3.5 `std/dispatch/dispatcher/dispatch_executor_adapter.rs` を削除する
- [ ] 3.6 `std.rs` の dispatch モジュール宣言から削除したファイルの参照を除去する（`pub use base::*`, `pub use dispatch_executor_adapter::*`）
- [ ] 3.7 `std/dispatch/dispatcher/base/tests.rs` を更新または削除する
- [ ] 3.8 crate 内で `std::dispatch::dispatcher::DispatchExecutor` を参照している全箇所を `core::dispatch::dispatcher::DispatchExecutor` に置き換える
- [ ] 3.9 コンパイルとテストが通ることを確認する

## 4. quickstart 廃止と new() デフォルト化（Capability C）

- [ ] 4.1 `std/scheduler/tick.rs` で `tokio_quickstart()` のロジックを `Default` trait 実装に移行する（`impl Default for TickDriverConfig` を `#[cfg(feature = "tokio-executor")]` 下で定義）
- [ ] 4.2 `tokio_quickstart_with_resolution()` を `with_resolution()` にリネームする
- [ ] 4.3 `std/dispatch/dispatcher/dispatcher_config.rs` で `tokio_auto()` のロジックを `Default` trait 実装に移行する（`impl Default for DispatcherConfig` を `#[cfg(feature = "tokio-executor")]` 下で定義）
- [ ] 4.4 `std/system/base.rs` の `quickstart()` と `quickstart_with()` を削除する
- [ ] 4.5 `std/system/base.rs` の `new()` を変更: `#[cfg(feature = "tokio-executor")]` 時に `TickDriverConfig::default()` と `DispatcherConfig::default()` を使ってデフォルト構成する
- [ ] 4.6 `modules/actor/examples/` の全 example を更新: `quickstart()` → `new()`、`tokio_quickstart()` → `TickDriverConfig::default()` 等
- [ ] 4.7 `modules/actor/benches/actor_baseline.rs` を更新する
- [ ] 4.8 `modules/cluster/examples/` の全 example を更新する
- [ ] 4.9 `modules/remote/examples/` と `modules/remote/tests/` を更新する
- [ ] 4.10 `std/system/base/tests.rs` を更新する
- [ ] 4.11 `tokio_quickstart()` と `tokio_auto()` の旧メソッドを削除する

## 5. 最終検証

- [ ] 5.1 `./scripts/ci-check.sh ai all` を実行し、全テスト・lint が通ることを確認する
- [ ] 5.2 `std/` に残ったファイルがすべてプラットフォーム依存を持つことを確認する（no_std 互換のロジックが std/ に残っていないこと）
