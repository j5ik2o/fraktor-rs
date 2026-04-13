## 1. 新 TickDriver trait + TickDriverStopper + TickDriverProvision 定義

- [ ] 1.1 actor-core に新 `TickDriver` trait を定義する（`provision(self: Box<Self>, feed, executor) -> Result<TickDriverProvision, _>`）
- [ ] 1.2 `TickDriverStopper` trait を定義する（`stop(self: Box<Self>)` — join 可能な停止契約）
- [ ] 1.3 `TickDriverProvision` 構造体を定義する（`resolution`, `id`, `kind`, `stopper`, `auto_metadata` — snapshot 互換）
- [ ] 1.4 `TickDriverKind` に `#[non_exhaustive]` を付与し、`Std` variant を追加する
- [ ] 1.5 旧 `TickDriver` trait との共存のためモジュール分離を行う

## 2. ActorSystemConfig + ActorSystem + ActorSystemSetup 新 API

- [ ] 2.1 `ActorSystemConfig` に `tick_driver: Option<Box<dyn TickDriver>>` フィールドを追加する（新 trait 版。旧 `tick_driver_config` は残す）
- [ ] 2.2 `ActorSystemConfig::with_new_tick_driver(impl TickDriver + 'static)` を追加する（新 trait 版。旧 `with_tick_driver(TickDriverConfig)` は名前・シグネチャを変更せずそのまま残す）
- [ ] 2.3 `ActorSystem::create_with_config(props, config: ActorSystemConfig)` を追加する（config を消費）
- [ ] 2.4 `TypedActorSystem::create_with_config` を追加する（薄い皮）
- [ ] 2.5 `ActorSystemSetup::with_new_tick_driver(impl TickDriver + 'static)` を追加する
- [ ] 2.6 `ActorSystem::create_with_setup(props, setup: ActorSystemSetup)` を追加する
- [ ] 2.7 bootstrap の新経路を実装する（config から driver を `.take()` → `provision` で消費 → `TickDriverSnapshot` を構築）

## 3. StdTickDriver 新設

- [ ] 3.1 `modules/actor-adaptor-std/src/std/tick_driver/std_tick_driver.rs` を新設する
- [ ] 3.2 `StdTickDriver` を実装する（`impl TickDriver` — `std::thread` + `sleep` で tick 生成 + executor 駆動）
- [ ] 3.3 `StdTickDriverStopper` を実装する（`AtomicBool` + `JoinHandle::join()` で完全停止）
- [ ] 3.4 `tick_driver.rs`（既存モジュールファイル）に `mod std_tick_driver` と re-export を追加する

## 4. テスト用 driver 新設

- [ ] 4.1 新 `TickDriver` trait 用のテスト driver を新設する（旧 `ManualTestDriver` は触らない）
- [ ] 4.2 テスト driver 用の `runner_api_enabled` 自動有効化パスを新 API 側に実装する

## 5. 検証

- [ ] 5.1 `cargo check --lib --workspace` がクリーンにビルドされることを確認する
- [ ] 5.2 `cargo check --tests --workspace` がクリーンにビルドされることを確認する（旧 API のテストがそのまま通ること）
- [ ] 5.3 showcase の getting_started を新 API で動作確認する（`StdTickDriver::default()` のみで起動）
- [ ] 5.4 `./scripts/ci-check.sh` が全パスすることを確認する
