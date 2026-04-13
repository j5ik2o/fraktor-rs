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
let config = ActorSystemConfig::default()
    .with_new_tick_driver(StdTickDriver::default())
    .with_dispatcher_configurator(id, configurator);

ActorSystem::create_with_config(&props, config)?;
```

tick driver も dispatcher も extension も全部 `ActorSystemConfig` の builder 経由。特別扱いなし。

## What Changes

### Phase 1: 新 port 契約 + std adapter + 新 API（本 change のスコープ）

1. **actor-core に新しい `TickDriver` trait を定義する**

   `provision(self: Box<Self>, feed, executor) -> Result<TickDriverProvision, _>` — 単一メソッド、`Box<Self>` で所有権消費かつ object-safe。tick 生成と executor 駆動をどう orchestrate するかは実装者の自由。

2. **`TickDriverStopper` trait を新設する**

   `stop(self: Box<Self>)` — 所有権を取って join 可能。旧 `TickDriverControl::shutdown(&self)` では thread join を待てない問題を解決。

3. **`ActorSystem::create_with_config` を新設する**

   `create_with_config(props, config: ActorSystemConfig)` — config を消費する。`Option<Box<dyn TickDriver>>` を `.take()` で取り出して move で消費。旧 `new_with_config(&props, &config)` は残す。tick driver だけ別引数にする shortcut は設けない — config 経由に統一。

4. **actor-adaptor-std に `StdTickDriver` を新設する**

   `std::thread` + `sleep` で tick 生成 + executor 駆動を行う adapter。`TickPulseSource` / `HardwareTickDriver` を経由しない。

5. **テスト用 driver を新設する**

   旧 `ManualTestDriver` は触らない。新 `TickDriver` trait 用のテスト driver を新しく作る。

### Phase 2: 移行（別 change）

- showcase + テスト群を新 API に移行
- 旧 API を deprecated 化

### Phase 3: 旧設計の削除（別 change）

- 旧 `TickDriver` trait / `TickExecutorPump` trait / `TickDriverConfig` enum の削除
- `HardwareTickDriver` / `TickPulseSource` / `ManualTestDriver` の削除
- 旧 `new` / `new_with_config` の削除

## Capabilities

### New Capabilities
- `tick-driver-unified-trait`: tick 生成と executor 駆動を 1 つの trait で表現
- `tick-driver-stopper`: join 可能な停止契約
- `std-tick-driver`: `std::thread` ベースの tick driver adapter

### Modified Capabilities
- `actor-system-config-api`: `create_with_config` が config を消費する（`&config` → `config`）

## Impact

- 対象コード:
  - `modules/actor-core/src/core/kernel/actor/scheduler/tick_driver/` — 新 trait 定義（`TickDriver`, `TickDriverStopper`, `TickDriverProvision`）
  - `modules/actor-core/src/core/kernel/system/base.rs` — `create_with_config` / `create_with_setup` 追加
  - `modules/actor-core/src/core/typed/system.rs` — `create_with_config` 追加
  - `modules/actor-core/src/core/kernel/actor/setup/actor_system_config.rs` — `with_tick_driver(impl TickDriver)` + `tick_driver: Option<Box<dyn TickDriver>>` 追加
  - `modules/actor-core/src/core/kernel/actor/setup/actor_system_setup.rs` — `with_new_tick_driver` + `create_with_setup` 追加
  - `modules/actor-adaptor-std/src/std/tick_driver/std_tick_driver.rs` — 新設
- 破壊的変更:
  - Phase 1 は基本的に additive だが、`TickDriverKind` への `#[non_exhaustive]` 付与 + `Std` variant 追加は既存の網羅的 `match` を壊す破壊的変更。旧 API の名前・シグネチャは変更しない
  - Phase 3 で旧 API を削除する際に破壊的変更となる
