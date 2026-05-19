## 1. CircuitBreaker の core 移設（Capability A）

- [x] 1.1 `core/pattern/clock.rs` に `Clock` trait を定義する（`type Instant`, `now()`, `elapsed_since()`）
- [x] 1.2 `std/pattern/std_clock.rs` に `StdClock` 構造体を作成し `Clock` trait を実装する（`std::time::Instant` ベース）
- [x] 1.3 `std/pattern/circuit_breaker_state.rs` を `core/pattern/circuit_breaker_state.rs` に移動する
- [x] 1.4 `std/pattern/circuit_breaker_call_error.rs` を `core/pattern/circuit_breaker_call_error.rs` に移動する
- [x] 1.5 `std/pattern/circuit_breaker_open_error.rs` を `core/pattern/circuit_breaker_open_error.rs` に移動する
- [x] 1.6 `std/pattern/circuit_breaker.rs` を `core/pattern/circuit_breaker.rs` に移動し、`CircuitBreaker<C: Clock>` にジェネリック化する（`Box<dyn Fn() -> Instant>` を `Clock` trait に置き換え）
- [x] 1.7 `std/pattern/circuit_breaker_shared.rs` を `core/pattern/circuit_breaker_shared.rs` に移動し、`CircuitBreakerShared<C: Clock>` にジェネリック化する
- [x] 1.8 `core/pattern.rs` のモジュール宣言に新規モジュールを追加し、公開型を re-export する
- [x] 1.9 `std/pattern.rs` を更新: core 型の re-export + `StdClock` を使った型エイリアス（`type CircuitBreaker = core::pattern::CircuitBreaker<StdClock>` 等）+ ファクトリ関数
- [x] 1.10 `std/pattern/circuit_breaker/tests.rs` と `std/pattern/circuit_breaker_shared/tests.rs` を移動・更新し、テストが通ることを確認する
- [x] 1.11 `core/pattern/` に Clock のテスト（FakeClock を使った CircuitBreaker のユニットテスト）を追加する

## 2. CoordinatedShutdown 型の core 移設（Capability D）

- [x] 2.1 ~~`std/system/coordinated_shutdown_error.rs` を `core/system/coordinated_shutdown_error.rs` に移動する~~ → std に残す（`std::error::Error` 実装のため）
- [x] 2.2 ~~`std/system/coordinated_shutdown_id.rs` を `core/system/coordinated_shutdown_id.rs` に移動する~~ → std に残す（`ExtensionId<CoordinatedShutdown>` が tokio 依存型を参照するため）
- [x] 2.3 `std/system/coordinated_shutdown_phase.rs` を `core/system/coordinated_shutdown_phase.rs` に移動する
- [x] 2.4 `std/system/coordinated_shutdown_reason.rs` を `core/system/coordinated_shutdown_reason.rs` に移動する
- [x] 2.5 `core/system.rs` のモジュール宣言に追加し、公開型を re-export する
- [x] 2.6 `std.rs` の system セクションを更新: Phase と Reason を core から re-export し、`#[cfg(feature = "tokio-executor")]` gate を外す
- [x] 2.7 `std/system/coordinated_shutdown.rs` の import パスを core に更新する
- [x] 2.8 コンパイルとテストが通ることを確認する

## 3. DispatchExecutor の統合（Capability B）

- [x] 3.1 `std/dispatch/dispatcher/dispatch_executor/tokio_executor.rs` を変更: core trait を直接実装する
- [x] 3.2 `std/dispatch/dispatcher/dispatch_executor/thread_executor.rs` を同様に変更する
- [x] 3.3 `std/dispatch/dispatcher/dispatcher_config.rs` を変更: executor の受け取りを `Box<dyn core::DispatchExecutor>` に統一する
- [x] 3.4 `std/dispatch/dispatcher/base.rs` の std 版 `DispatchExecutor` trait を削除する
- [x] 3.5 `std/dispatch/dispatcher/dispatch_executor_adapter.rs` を削除する
- [x] 3.6 `std.rs` の dispatch モジュール宣言から削除したファイルの参照を除去する
- [x] 3.7 `std/dispatch/dispatcher/base/tests.rs` を削除する（std trait 廃止により不要）
- [x] 3.8 crate 内および examples で `from_executor(ArcShared<StdSyncMutex<...>>)` を `from_executor(Box<...>)` に置き換える
- [x] 3.9 コンパイルとテストが通ることを確認する（186テスト全パス）

## 4. quickstart 廃止と new() デフォルト化（Capability C）

- [x] 4.1 `std/scheduler/tick.rs` に `default_config()` を追加、`tokio_quickstart()` を deprecated にする
- [x] 4.2 `tokio_quickstart_with_resolution()` を `with_resolution()` にリネームする
- [x] 4.3 `std/dispatch/dispatcher/dispatcher_config.rs` に `default_config()` を追加、`tokio_auto()` を deprecated にする
- [x] 4.4 `std/system/base.rs` の `quickstart()` と `quickstart_with()` を削除する
- [x] 4.5 `std/system/base.rs` の `new()` を1引数に変更: デフォルトで TickDriver + Dispatcher を自動構成。旧2引数版は `new_with_tick_driver()` にリネーム
- [x] 4.6 `modules/actor/examples/` の全 std example を更新: `quickstart()` → `new()`、`new(props, tick)` → `new_with_tick_driver(props, tick)`、`tokio_quickstart()` → `default_config()`
- [x] 4.7 `modules/actor/benches/actor_baseline.rs` を更新する
- [x] 4.8 `modules/cluster/examples/` の全 example を更新する
- [x] 4.9 `modules/remote/examples/` を更新する
- [x] 4.10 `std/system/base/tests.rs` を更新する
- [ ] 4.11 `tokio_quickstart()` と `tokio_auto()` の deprecated メソッドを削除する（次回以降）

## 5. 最終検証

- [x] 5.1 `./scripts/ci-check.sh ai all` 全パス（エラー 0 件）
- [ ] 5.2 `std/` に残ったファイルがすべてプラットフォーム依存を持つことを確認する（no_std 互換のロジックが std/ に残っていないこと）
