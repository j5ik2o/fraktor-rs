## 1. 新 TickDriver trait 定義 + 旧 trait 置き換え

- [ ] 1.1 `tick_driver_trait.rs` の旧 `TickDriver` trait を新 trait に置き換える（`provision(self: Box<Self>, feed, executor) -> Result<TickDriverProvision, _>`）
- [ ] 1.2 `TickDriverStopper` trait を定義する（`stop(self: Box<Self>)` — join 可能な停止契約）
- [ ] 1.3 `TickDriverProvision` 構造体を定義する（`resolution`, `id`, `kind`, `stopper`, `auto_metadata` — snapshot 互換）
- [ ] 1.4 `TickDriverKind` に `#[non_exhaustive]` を付与し、`Std` variant を追加する
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

## 4. テスト用 driver 新設

- [ ] 4.1 新 `TickDriver` trait 用のテスト driver を新設する
- [ ] 4.2 テスト driver 用の `runner_api_enabled` 自動有効化パスを新 API 側に実装する

## 5. showcase + テスト群の新 API 移行

- [ ] 5.1 `showcases/std/src/support/tick_driver.rs` の旧 `hardware_tick_driver_config()` を `StdTickDriver` ベースに書き換える（旧 `DemoPulse` / `StdTickExecutorPump` / `HardwareTickDriver` 経由のコードを削除）
- [ ] 5.2 `showcases/std/getting_started/main.rs` を新 API（`ActorSystemConfig::new(StdTickDriver::default())` + `TypedActorSystem::create_with_config`）に移行する
- [ ] 5.3 `showcases/std/child_lifecycle/main.rs` を新 API に移行する
- [ ] 5.4 `showcases/std/state_management/main.rs` を新 API に移行する
- [ ] 5.5 `showcases/std/request_reply/main.rs` を新 API に移行する
- [ ] 5.6 `showcases/std/stash/main.rs` を新 API に移行する
- [ ] 5.7 `showcases/std/timers/main.rs` を新 API に移行する
- [ ] 5.8 `showcases/std/classic_timers/main.rs` を新 API に移行する
- [ ] 5.9 `showcases/std/classic_logging/main.rs` を新 API に移行する
- [ ] 5.10 `showcases/std/routing/main.rs` を新 API に移行する
- [ ] 5.11 `showcases/std/serialization/main.rs` を新 API に移行する
- [ ] 5.12 `showcases/std/persistent_actor/main.rs` を新 API に移行する
- [ ] 5.13 `showcases/std/typed_receptionist_router/main.rs` を新 API に移行する
- [ ] 5.14 `showcases/std/typed_event_stream/main.rs` を新 API に移行する
- [ ] 5.15 `showcases/std/stream_pipeline/main.rs` を新 API に移行する
- [ ] 5.16 `showcases/std/tests/routing_surface.rs` を新 API に移行する
- [ ] 5.17 `showcases/std/tests/shared_lock_showcase_surface.rs` を新 API に移行する

## 6. 検証

- [ ] 6.1 `cargo check --lib --workspace` がクリーンにビルドされることを確認する
- [ ] 6.2 `cargo check --tests --workspace` がクリーンにビルドされることを確認する
- [ ] 6.3 全 showcase が新 API で動作することを確認する
- [ ] 6.4 `./scripts/ci-check.sh` が全パスすることを確認する
