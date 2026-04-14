## 1. 新 TickDriver trait 定義 + 旧 trait 置き換え

- [ ] 1.1 `tick_driver_trait.rs` の旧 `TickDriver` trait を新 trait に置き換える（`provision(self: Box<Self>, feed, executor) -> Result<TickDriverProvision, _>`）
- [ ] 1.2 `TickDriverStopper` trait を定義する（`stop(self: Box<Self>)` — join 可能な停止契約）
- [ ] 1.3 `TickDriverProvision` 構造体を定義する（`resolution`, `id`, `kind`, `stopper`, `auto_metadata` — snapshot 互換）
- [ ] 1.4 `TickDriverKind` に `#[non_exhaustive]` を付与し、`Std` / `Tokio` variant を追加する
- [ ] 1.5 `next_tick_driver_id()` を `tick_driver_trait.rs` から `tick_driver_id.rs` に移動する
- [ ] 1.6 旧 `TickDriverConfig` / `TickExecutorPump` / `HardwareTickDriver` / `TickPulseSource` / `ManualTestDriver` / `TickDriverControl` を削除する

## 2. ActorSystemConfig + ActorSystem + ActorSystemSetup API 置き換え

- [ ] 2.1 `ActorSystemConfig` の旧 `tick_driver_config: Option<TickDriverConfig>` フィールドを `tick_driver: Option<Box<dyn TickDriver>>` に置き換える
- [ ] 2.2 `ActorSystemConfig::new(impl TickDriver + 'static)` を追加する（TickDriver を必須引数にする推奨コンストラクタ）
- [ ] 2.3 旧 `ActorSystemConfig::with_tick_driver(TickDriverConfig)` を `with_tick_driver(impl TickDriver + 'static)` に置き換える
- [ ] 2.4 `ActorSystemConfig::take_tick_driver(&mut self) -> Option<Box<dyn TickDriver>>` を追加する
- [ ] 2.5 `ActorSystem::create_with_config_and(props, config, configure)` を追加する（config を消費 + 拡張コールバック。新 API の core メソッド）
- [ ] 2.6 `ActorSystem::create_with_config(props, config)` を追加する（`create_with_config_and` に委譲）
- [ ] 2.7 `TypedActorSystem::create_with_config` を追加する（薄い皮）
- [ ] 2.8 旧 `ActorSystemSetup::with_tick_driver(TickDriverConfig)` を `with_tick_driver(impl TickDriver + 'static)` に置き換える
- [ ] 2.9 `ActorSystem::create_with_setup(props, setup: ActorSystemSetup)` を追加する（`create_with_config_and` に委譲）
- [ ] 2.10 旧 `SystemState::build_from_config(&ActorSystemConfig)` を `build_from_owned_config(config: ActorSystemConfig)` に置き換える（config を move で受け取り、`tick_driver.take()` → `provision` で起動）
- [ ] 2.11 旧 API を削除する（`ActorSystem::new` / `new_with_config` / `new_with_config_and` / `new_with_setup`）

## 3. StdTickDriver 新設

- [ ] 3.1 `modules/actor-adaptor-std/src/std/tick_driver/std_tick_driver.rs` を新設する
- [ ] 3.2 `StdTickDriver` を実装する（`impl TickDriver` — `std::thread` + `sleep` で tick 生成 + executor 駆動）
- [ ] 3.3 `StdTickDriverStopper` を実装する（`AtomicBool` + `JoinHandle::join()` で完全停止）
- [ ] 3.4 `tick_driver.rs`（既存モジュールファイル）に `mod std_tick_driver` と re-export を追加する

## 4. TokioTickDriver 新設（旧 Tokio 実装の新 trait 移行）

- [ ] 4.1 `modules/actor-adaptor-std/src/std/tick_driver/tokio_tick_driver.rs` を新設する（`#[cfg(feature = "tokio-executor")]`）
- [ ] 4.2 `TokioTickDriver` を実装する（`impl TickDriver` — `tokio::time::interval` で tick 生成 + `tokio::time::sleep` で executor 駆動）
- [ ] 4.3 `TokioTickDriverStopper` を実装する（`JoinHandle::abort()` で停止）
- [ ] 4.4 `tick_driver.rs`（既存モジュールファイル）に `mod tokio_tick_driver` と re-export を追加する
- [ ] 4.5 旧 Tokio 実装（`TokioTickDriver` / `TokioTickExecutorPump` / `TokioTickDriverControl` / `TokioTickExecutorControl` / `default_tick_driver_config` / `tick_driver_config_with_resolution`）を `tick_driver.rs` から削除する

## 5. テスト用 driver 新設

- [ ] 5.1 `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/test_tick_driver.rs` に新 `TickDriver` trait 用のテスト driver を新設する（旧 `manual_test_driver.rs` を置き換え）
- [ ] 5.2 テスト driver 用の `runner_api_enabled` 自動有効化パスを新 API 側に実装する

## 6. showcase + テスト群 + bench の新 API 移行

### 6.A showcase 移行

- [ ] 6.1 `showcases/std/src/support/tick_driver.rs` の旧 `hardware_tick_driver_config()` を `StdTickDriver` ベースに書き換える（旧 `DemoPulse` / `StdTickExecutorPump` / `HardwareTickDriver` 経由のコードを削除。旧 `tokio_tick_driver_config` 系も `TokioTickDriver` ベースに置き換える）
- [ ] 6.2 `showcases/std/src/support/materializer.rs` を新 API に移行する（旧 `ManualTestDriver` / `TickDriverConfig::manual` → 新テスト driver）
- [ ] 6.3 `showcases/std/getting_started/main.rs` を新 API（`ActorSystemConfig::new(StdTickDriver::default())` + `TypedActorSystem::create_with_config`）に移行する
- [ ] 6.4 `showcases/std/child_lifecycle/main.rs` を新 API に移行する
- [ ] 6.5 `showcases/std/state_management/main.rs` を新 API に移行する
- [ ] 6.6 `showcases/std/request_reply/main.rs` を新 API に移行する
- [ ] 6.7 `showcases/std/stash/main.rs` を新 API に移行する
- [ ] 6.8 `showcases/std/timers/main.rs` を新 API に移行する
- [ ] 6.9 `showcases/std/classic_timers/main.rs` を新 API に移行する
- [ ] 6.10 `showcases/std/classic_logging/main.rs` を新 API に移行する
- [ ] 6.11 `showcases/std/routing/main.rs` を新 API に移行する
- [ ] 6.12 `showcases/std/serialization/main.rs` を新 API に移行する
- [ ] 6.13 `showcases/std/persistent_actor/main.rs` を新 API に移行する
- [ ] 6.14 `showcases/std/typed_receptionist_router/main.rs` を新 API に移行する
- [ ] 6.15 `showcases/std/typed_event_stream/main.rs` を新 API に移行する
- [ ] 6.16 `showcases/std/stream_pipeline/main.rs` を新 API に移行する
- [ ] 6.17 `showcases/std/tests/routing_surface.rs` を新 API に移行する
- [ ] 6.18 `showcases/std/tests/shared_lock_showcase_surface.rs` を新 API に移行する

### 6.B actor-core ワークスペーステスト移行

- [ ] 6.19 `modules/actor-core/tests/death_watch.rs` を新 API に移行する（旧 `ActorSystem::new` → `create_with_config`）
- [ ] 6.20 `modules/actor-core/tests/system_events.rs` を新 API に移行する
- [ ] 6.21 `modules/actor-core/tests/supervisor.rs` を新 API に移行する
- [ ] 6.22 `modules/actor-core/tests/event_stream.rs` を新 API に移行する
- [ ] 6.23 `modules/actor-core/tests/ping_pong.rs` を新 API に移行する
- [ ] 6.24 `modules/actor-core/tests/system_lifecycle.rs` を新 API に移行する

### 6.C actor-core 内部テスト移行

- [ ] 6.25 `modules/actor-core/src/core/kernel/system/base/tests.rs` を新 API に移行する（旧 `TickDriverConfig` / `ManualTestDriver` / `new_with_config_and` → 新テスト driver + `create_with_config_and`）
- [ ] 6.26 `modules/actor-core/src/core/kernel/actor/actor_ref_provider/local_actor_ref_provider/tests.rs` を新 API に移行する
- [ ] 6.27 `modules/actor-core/src/core/kernel/actor/actor_selection/tests.rs` を新 API に移行する
- [ ] 6.28 `modules/actor-core/src/core/kernel/actor/actor_ref/tests.rs` を新 API に移行する
- [ ] 6.29 `modules/actor-core/src/core/kernel/dispatch/mailbox/tests.rs` を新 API に移行する
- [ ] 6.30 `modules/actor-core/src/core/kernel/system/state/system_state/tests.rs` を新 API に移行する
- [ ] 6.31 `modules/actor-core/src/tests.rs` を新 API に移行する（旧型の import / TypeId テストを更新）
- [ ] 6.32 `modules/actor-core/src/core/typed/system/tests.rs` を新 API に移行する（旧 `TickDriverConfig::manual` / `TypedActorSystem::new_with_config` → 新テスト driver + `create_with_config`）
- [ ] 6.33 `modules/actor-core/src/core/typed/actor_ref_resolver/tests.rs` を新 API に移行する（旧 `TypedActorSystem::new_with_config` → `create_with_config`）
- [ ] 6.34 `modules/actor-core/src/core/typed/extension_setup/tests.rs` を新 API に移行する（旧 `TypedActorSystem::new_with_config` → `create_with_config`）
- [ ] 6.35 `modules/actor-core/src/core/kernel/actor/setup/actor_system_setup/tests.rs` を新 API に移行する（旧 `ActorSystemSetup::with_tick_driver(TickDriverConfig)` → `with_tick_driver(新テスト driver)` + `create_with_setup`）

### 6.D actor-adaptor-std テスト + bench 移行

- [ ] 6.36 `modules/actor-adaptor-std/src/std/dispatch/dispatcher/tests.rs` を新 API に移行する（旧 `default_tick_driver_config` → `TokioTickDriver`）
- [ ] 6.37 `modules/actor-adaptor-std/src/std/tick_driver/tests.rs` を新 API に移行する（旧 trait テスト → 新 `TokioTickDriver` テスト）
- [ ] 6.38 `modules/actor-adaptor-std/benches/actor_baseline.rs` を新 API に移行する（旧 `with_tick_driver(default_tick_driver_config())` / `new_with_config` → `TokioTickDriver` + `create_with_config`）
- [ ] 6.39 `modules/actor-adaptor-std/benches/balancing_dispatcher.rs` を新 API に移行する

### 6.E persistence-core テスト移行

- [ ] 6.40 `modules/persistence-core/src/core/persistent_actor_adapter/tests.rs` を新 API に移行する（旧 `ManualTestDriver` / `TickDriverConfig::manual` / `new_with_config` → 新テスト driver + `create_with_config`）
- [ ] 6.41 `modules/persistence-core/src/core/persistence_extension/tests.rs` を新 API に移行する
- [ ] 6.42 `modules/persistence-core/src/core/persistence_extension_installer/tests.rs` を新 API に移行する
- [ ] 6.43 `modules/persistence-core/tests/persistence_flow.rs` を新 API に移行する
- [ ] 6.44 `modules/persistence-core/tests/persistent_actor_example.rs` を新 API に移行する

### 6.F cluster-core テスト移行

- [ ] 6.45 `modules/cluster-core/src/core/cluster_api/tests.rs` を新 API に移行する（旧 `with_tick_driver` / `new_with_config` → 新テスト driver + `create_with_config`）
- [ ] 6.46 `modules/cluster-core/src/core/grain/grain_ref/tests.rs` を新 API に移行する
- [ ] 6.47 `modules/cluster-core/src/core/grain/grain_context_scope/tests.rs` を新 API に移行する
- [ ] 6.48 `modules/cluster-core/src/core/grain/grain_context_generic/tests.rs` を新 API に移行する

### 6.G stream-core テスト移行

- [ ] 6.49 `modules/stream-core/tests/requirement_traceability.rs` を新 API に移行する
- [ ] 6.50 `modules/stream-core/src/core/materialization/actor_materializer/tests.rs` を新 API に移行する
- [ ] 6.51 `modules/stream-core/src/core/dsl/topic_pub_sub/tests.rs` を新 API に移行する

### 6.H 旧 adaptor-std re-export 削除

- [ ] 6.52 `modules/actor-adaptor-std/src/std.rs` の旧 re-export（`default_tick_driver_config` / `tick_driver_config_with_resolution`）を `TokioTickDriver` の re-export に置き換える

## 7. 検証

- [ ] 7.1 `cargo check --lib --workspace` がクリーンにビルドされることを確認する
- [ ] 7.2 `cargo check --tests --workspace` がクリーンにビルドされることを確認する
- [ ] 7.3 `cargo check --benches --workspace` がクリーンにビルドされることを確認する
- [ ] 7.4 全 showcase が新 API で動作することを確認する
- [ ] 7.5 `./scripts/ci-check.sh` が全パスすることを確認する
