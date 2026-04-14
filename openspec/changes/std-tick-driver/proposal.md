## Why

`TypedActorSystem::new` でユーザが tick driver を組み立てるために必要な知識が多すぎる:

```rust
// 現状 — showcase の support ヘルパーに依存
let (tick_driver_config, _pulse_handle) = support::hardware_tick_driver_config();
let system = TypedActorSystem::new(&props, tick_driver_config)?;
```

根本原因:
1. actor-core の port 契約が「tick 生成」と「executor 駆動」を 2 つの別 trait に分割している
2. それを `TickDriverConfig` enum で束ね、`ActorSystemConfig` に格納している
3. `actor-adaptor-std` に `std::thread` ベースの adapter が存在せず、showcase の support に実装が逃げている

### あるべきユーザ体験

```rust
let config = ActorSystemConfig::new(StdTickDriver::default())
    .with_dispatcher_configurator(id, configurator);

// untyped API
ActorSystem::create_with_config(&props, config)?;
// または typed API（showcase で主に使用）
TypedActorSystem::create_with_config(&props, config)?;
```

`ActorSystemConfig::new(driver)` で TickDriver を必須引数にし、推奨パスを明示する。actor-core は no_std のためデフォルトの TickDriver を提供できない。dispatcher や extension は builder メソッドで追加。

## What Changes（すべて本 change のスコープ）

実装順序に沿って以下のステップで進める。すべて単一の change で完結する。

### Step 1: 新 trait 定義

1. **actor-core に新しい `TickDriver` trait を定義する**

   `kind(&self) -> TickDriverKind` で provision 前の種別判定を提供し、`provision(self: Box<Self>, feed, executor) -> Result<TickDriverProvision, _>` で所有権を消費して駆動を開始する。`Box<Self>` で object-safe。tick 生成と executor 駆動をどう orchestrate するかは実装者の自由。

2. **`TickDriverStopper` trait を新設する**

   `stop(self: Box<Self>)` — 所有権を取って join 可能。旧 `TickDriverControl::shutdown(&self)` では thread join を待てない問題を解決。

### Step 2: 新 API + StdTickDriver

3. **`ActorSystemConfig::new(driver)` + `ActorSystem::create_with_config_and` / `create_with_config` を新設する**

   `ActorSystemConfig::new(driver)` で TickDriver を必須引数にする推奨コンストラクタを追加。`create_with_config_and(props, config, configure)` が core メソッドで、config を消費し拡張コールバックを実行する。`create_with_config(props, config)` と `create_with_setup(props, setup)` はこれに委譲する。tick driver だけ別引数にする shortcut は設けない — config 経由に統一。

4. **actor-adaptor-std に `StdTickDriver` を新設する**

   `std::thread` + `sleep` で tick 生成 + executor 駆動を行う adapter。`TickPulseSource` / `HardwareTickDriver` を経由しない。

5. **actor-adaptor-std の旧 Tokio 実装を `TokioTickDriver` に置き換える**

   旧 `TickDriver` / `TickExecutorPump` / `TickDriverControl` の Tokio 実装を削除し、新 `TickDriver` trait の `TokioTickDriver` 実装に置き換える。`tokio::time::interval` ベース。`#[cfg(feature = "tokio-executor")]` を維持。

6. **テスト用 driver を新設する**

   新 `TickDriver` trait 用のテスト driver を新しく作る。

### Step 3: 移行 + 旧設計の削除

7. **showcase + テスト群 + bench を新 API に移行する**

   `showcases/std/` 配下の全 showcase、ワークスペース全体のテスト群（actor-core / actor-adaptor-std / persistence-core / cluster-core / stream-core）、bench を新 API（`ActorSystemConfig::new(StdTickDriver::default())` / `TokioTickDriver::default()` + `create_with_config`）に書き換える。

8. **旧設計を削除する**

   旧 `TickDriver` trait / `TickExecutorPump` trait / `TickDriverConfig` enum / `HardwareTickDriver` / `TickPulseSource` / `ManualTestDriver` / `TickDriverControl` / `TokioTickExecutorPump` / `TokioTickDriverControl` / `TokioTickExecutorControl` / 旧 `new` / `new_with_config` / `new_with_config_and` / `new_with_setup` / 旧 `default_tick_driver_config` / `tick_driver_config_with_resolution` を削除する。旧 `support::hardware_tick_driver_config()` の `DemoPulse` 等も削除する。

## Capabilities

### New Capabilities
- `tick-driver-unified-trait`: tick 生成と executor 駆動を 1 つの trait で表現
- `tick-driver-stopper`: join 可能な停止契約
- `std-tick-driver`: `std::thread` ベースの tick driver adapter
- `tokio-tick-driver`: `tokio::time::interval` ベースの tick driver adapter（旧 Tokio 実装の新 trait 移行）
- `test-tick-driver`: 新 `TickDriver` trait 用のテスト driver

### Modified Capabilities
- `actor-system-config-api`: `create_with_config` が config を消費する（`&config` → `config`）

## Impact

- 対象コード:
  - `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/` — 新 trait 定義（`TickDriver`, `TickDriverStopper`, `TickDriverProvision`）
  - `modules/actor-core/src/core/kernel/system/base.rs` — `create_with_config_and` / `create_with_config` / `create_with_setup` 追加
  - `modules/actor-core/src/core/kernel/system/state/system_state.rs` — 旧 `build_from_config(&ActorSystemConfig)` を `build_from_owned_config(config: ActorSystemConfig)` に置き換え
  - `modules/actor-core/src/core/typed/system.rs` — `create_with_config` 追加
  - `modules/actor-core/src/core/kernel/actor/setup/actor_system_config.rs` — `with_tick_driver(impl TickDriver + 'static)` + `tick_driver: Option<Box<dyn TickDriver>>` に置き換え
  - `modules/actor-core/src/core/kernel/actor/setup/actor_system_setup.rs` — `with_tick_driver` を新シグネチャに置き換え
  - `modules/actor-adaptor-std/src/std/tick_driver/std_tick_driver.rs` — 新設
  - `modules/actor-adaptor-std/src/std/tick_driver/tokio_tick_driver.rs` — 新設（旧 Tokio 実装を新 trait に移行）
  - `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/test_tick_driver.rs` — 新設（旧 `manual_test_driver.rs` を置き換え）
- 破壊的変更:
  - 旧 API（`new` / `new_with_config` / `new_with_config_and` / `new_with_setup`）を削除する
  - 旧 `TickDriver` trait / `TickDriverConfig` / `TickExecutorPump` / `HardwareTickDriver` / `TickPulseSource` / `ManualTestDriver` / `TickDriverControl` / `TokioTickExecutorPump` / `TokioTickDriverControl` / `TokioTickExecutorControl` を削除する
  - 旧 `default_tick_driver_config` / `tick_driver_config_with_resolution` ヘルパー関数を削除する
  - `TickDriverKind` に `#[non_exhaustive]` を付与し `Std` / `Tokio` variant を追加する
  - `TickDriverError` に `UnsupportedRuntime` variant を追加する（`TokioTickDriver` が current-thread runtime を拒否するために使用）
