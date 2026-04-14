## ADDED Requirements

### Requirement: TickDriver trait は provision(self: Box<Self>) 1 メソッドで object-safe に消費する

`TickDriver` trait は `provision(self: Box<Self>, feed, executor)` メソッド 1 つで tick 生成と executor 駆動の両方を開始しなければならない（MUST）。`self: Box<Self>` により object safety を維持しつつ所有権を消費する。`provision_boxed` のような 2 メソッド構成にしてはならない（MUST NOT）。

#### Scenario: Box<dyn TickDriver> から provision を呼べる

- **GIVEN** `ActorSystemConfig` に `Box<dyn TickDriver>` として格納された driver
- **WHEN** bootstrap が `driver.provision(feed, executor)` を呼ぶ
- **THEN** `self: Box<Self>` dispatch により object-safe に呼び出せる
- **AND** provision 後に driver は消費済みで再利用不可

### Requirement: TickDriverProvision は snapshot 互換の情報を返す

`TickDriverProvision` は `resolution`, `id`, `kind`, `stopper`, `auto_metadata` を含まなければならない（MUST）。bootstrap はこの情報から `TickDriverSnapshot` を構築して event stream に publish する。

#### Scenario: provision の戻り値から TickDriverSnapshot を構築できる

- **GIVEN** `TickDriver::provision` が `TickDriverProvision` を返す
- **WHEN** bootstrap がその戻り値を受け取る
- **THEN** `id`, `kind`, `resolution`, `auto_metadata` から `TickDriverSnapshot` を構築できる
- **AND** `stopper` から `TickDriverStopper` を取得できる

### Requirement: TickDriverStopper は所有権を取って join 可能でなければならない

`TickDriverStopper::stop(self: Box<Self>)` は所有権を消費し、全スレッド/タスクの完了を待って返らなければならない（MUST）。

#### Scenario: stop 後にスレッドが完全に停止している

- **GIVEN** `StdTickDriver` が起動中
- **WHEN** `stopper.stop()` が呼ばれる
- **THEN** `stop` が返った時点で tick thread と executor thread は join 済み
- **AND** 返却後に feed / executor へのアクセスは一切発生しない

### Requirement: ActorSystemConfig の builder に統一的に組み込む

tick driver は `ActorSystemConfig::with_tick_driver(impl TickDriver + 'static)` で config に格納しなければならない（MUST）。tick driver だけ別引数にしてはならない（MUST NOT）。

#### Scenario: config builder で tick driver を設定する

- **WHEN** `ActorSystemConfig::default().with_tick_driver(StdTickDriver::default())` が呼ばれる
- **THEN** config 内に `Box<dyn TickDriver>` として格納される
- **AND** `create_with_config(props, config)` で config を消費し、内部で `.take()` して driver を move できる

### Requirement: ActorSystemConfig::new(driver) で推奨パスを提供する

`ActorSystemConfig::new(impl TickDriver + 'static)` コンストラクタを提供しなければならない（MUST）。actor-core は no_std のためデフォルトの TickDriver を提供できないため、ユーザに environment adapter を渡させる推奨パスとする。

#### Scenario: new(driver) で config を生成する

- **WHEN** `ActorSystemConfig::new(StdTickDriver::default())` が呼ばれる
- **THEN** config 内に `Box<dyn TickDriver>` として格納される
- **AND** 他のフィールドはデフォルト値で初期化される

### Requirement: create_with_config_and が新 API の core メソッドである

`ActorSystem::create_with_config_and(props, config, configure)` を提供しなければならない（MUST）。`create_with_config` と `create_with_setup` はこのメソッドに委譲しなければならない（MUST）。`configure` コールバックで extension 登録等の拡張点を提供する。

#### Scenario: create_with_config_and で拡張コールバックを実行できる

- **GIVEN** `ActorSystemConfig::new(StdTickDriver::default())` で config が生成される
- **WHEN** `ActorSystem::create_with_config_and(props, config, |system| { /* extension 登録 */ Ok(()) })` が呼ばれる
- **THEN** config を消費してシステムが構築される
- **AND** `configure` コールバックが実行される

#### Scenario: create_with_config は create_with_config_and に委譲する

- **WHEN** `ActorSystem::create_with_config(props, config)` が呼ばれる
- **THEN** 内部で `create_with_config_and(props, config, |_| Ok(()))` に委譲される

#### Scenario: create_with_setup は create_with_config_and に委譲する

- **WHEN** `ActorSystem::create_with_setup(props, setup)` が呼ばれる
- **THEN** 内部で `setup.into_actor_system_config()` → `create_with_config_and` に委譲される

### Requirement: ActorSystemSetup も新 tick driver を受け付ける

`ActorSystemSetup` にも新 `TickDriver` を設定するメソッドを提供しなければならない（MUST）。Pekko 互換の setup 経路が旧設計に取り残されてはならない。

#### Scenario: setup facade で新 tick driver を設定する

- **WHEN** `ActorSystemSetup::default().with_tick_driver(StdTickDriver::default())` が呼ばれる
- **THEN** 内部の `ActorSystemConfig` に `Box<dyn TickDriver>` として格納される
- **AND** `ActorSystem::create_with_setup(props, setup)` で setup を消費してシステムが起動する

### Requirement: TypedActorSystem::create_with_config を提供する

`TypedActorSystem::create_with_config(props, config)` を提供しなければならない（MUST）。内部で `ActorSystem::create_with_config` に委譲する薄いラッパーとする。

#### Scenario: TypedActorSystem を create_with_config で起動できる

- **GIVEN** `ActorSystemConfig::new(StdTickDriver::default())` で config が生成される
- **WHEN** `TypedActorSystem::create_with_config(&props, config)` が呼ばれる
- **THEN** 内部で `ActorSystem::create_with_config` に委譲してシステムが構築される
- **AND** `TypedActorSystem` として型安全な API が利用可能

### Requirement: SystemState::build_from_owned_config は config を move で消費する

旧 `SystemState::build_from_config(&ActorSystemConfig)` を削除し、`build_from_owned_config(config: ActorSystemConfig)` に置き換えなければならない（MUST）。config を move で受け取り、内部で `tick_driver.take()` → `provision` で driver を起動する。

#### Scenario: build_from_owned_config が tick driver を provision する

- **GIVEN** `ActorSystemConfig::new(StdTickDriver::default())` で config が生成される
- **WHEN** `SystemState::build_from_owned_config(config)` が呼ばれる
- **THEN** config が move で消費される
- **AND** `tick_driver.take()` で driver が取り出される
- **AND** `driver.provision(feed, executor)` で tick 駆動が開始される
- **AND** driver が `None` の場合は `SpawnError::SystemBuildError` が返される

### Requirement: 旧 API は本 change で削除する

旧 `ActorSystem::new(props, TickDriverConfig)`, `new_with_config(&props, &config)`, `new_with_config_and(&props, &config, f)`, `new_with_setup(&props, &setup)` を削除しなければならない（MUST）。旧 `TickDriver` trait / `TickDriverConfig` / `TickExecutorPump` / `HardwareTickDriver` / `TickPulseSource` / `ManualTestDriver` / `TickDriverControl` / `TokioTickExecutorPump` / `TokioTickDriverControl` / `TokioTickExecutorControl` / `default_tick_driver_config` / `tick_driver_config_with_resolution` も削除しなければならない（MUST）。

#### Scenario: 旧 API が存在しない

- **GIVEN** 本 change が適用された状態
- **WHEN** `ActorSystem::new` や `new_with_config` を呼ぶコードをコンパイルする
- **THEN** コンパイルエラーになる（メソッドが存在しない）

#### Scenario: 全テスト・showcase が新 API で通る

- **GIVEN** 本 change が適用された状態
- **WHEN** `cargo check --tests --workspace` を実行する
- **THEN** 全テストがコンパイル・通過する（新 API に移行済み）

### Requirement: actor-adaptor-std は StdTickDriver を提供する

`actor-adaptor-std` は `std::thread` + `sleep` ベースの `TickDriver` 実装を提供しなければならない（MUST）。`TickPulseSource` / `HardwareTickDriver` の unsafe C ABI callback 機構を経由してはならない（MUST NOT）。

#### Scenario: StdTickDriver はデフォルト 10ms 解像度で動作する

- **GIVEN** `StdTickDriver::default()` が生成される
- **WHEN** `provision(self: Box<Self>, feed, executor)` が呼ばれる
- **THEN** `std::thread::spawn` で tick 生成スレッドと executor 駆動スレッドが起動される
- **AND** `resolution` は `Duration::from_millis(10)` が返される
- **AND** `kind` は `TickDriverKind::Std` が返される
- **AND** `auto_metadata` は `None` が返される
- **AND** `TickPulseSource::set_callback` は使用されない

#### Scenario: StdTickDriver はカスタム解像度で動作する

- **GIVEN** `StdTickDriver::new(Duration::from_millis(50))` が生成される
- **WHEN** `provision(self: Box<Self>, feed, executor)` が呼ばれる
- **THEN** tick 生成スレッドが 50ms 間隔で feed に tick を積む
- **AND** `resolution` は `Duration::from_millis(50)` が返される

#### Scenario: StdTickDriverStopper は全スレッドを join して停止する

- **GIVEN** `StdTickDriver` が provision 済みで 2 つのスレッドが稼働中
- **WHEN** `stopper.stop()` が呼ばれる
- **THEN** `AtomicBool` flag が false に設定される
- **AND** tick スレッドと executor スレッドが `JoinHandle::join()` で完了を待たれる
- **AND** `stop` が返った時点で両スレッドは完全に停止済み
- **AND** 返却後に feed / executor へのアクセスは一切発生しない

### Requirement: actor-adaptor-std は TokioTickDriver を提供する

`actor-adaptor-std` は `tokio::time::interval` ベースの `TickDriver` 実装を `#[cfg(feature = "tokio-executor")]` で提供しなければならない（MUST）。旧 `TickDriver` / `TickExecutorPump` / `TickDriverControl` の Tokio 実装を削除し、新 `TickDriver` trait の単一実装に置き換えなければならない（MUST）。

#### Scenario: TokioTickDriver はデフォルト 10ms 解像度で動作する

- **GIVEN** `TokioTickDriver::default()` が生成される
- **WHEN** Tokio runtime 内で `provision(self: Box<Self>, feed, executor)` が呼ばれる
- **THEN** `Handle::try_current()` で Tokio runtime handle を取得する
- **AND** `handle.spawn` で tick 生成 async task と executor 駆動 async task が起動される
- **AND** tick task は `tokio::time::interval(resolution)` で feed に tick を積む（`MissedTickBehavior::Delay` を設定）
- **AND** `resolution` は `Duration::from_millis(10)` が返される
- **AND** `kind` は `TickDriverKind::Tokio` が返される

#### Scenario: TokioTickDriver はカスタム解像度で動作する

- **GIVEN** `TokioTickDriver::new(Duration::from_millis(50))` が生成される
- **WHEN** Tokio runtime 内で `provision(self: Box<Self>, feed, executor)` が呼ばれる
- **THEN** tick 生成 task が 50ms 間隔で feed に tick を積む
- **AND** `resolution` は `Duration::from_millis(50)` が返される

#### Scenario: TokioTickDriver は Tokio runtime 外で HandleUnavailable エラーを返す

- **GIVEN** `TokioTickDriver::default()` が生成される
- **WHEN** Tokio runtime 外で `provision(self: Box<Self>, feed, executor)` が呼ばれる
- **THEN** `Handle::try_current()` が失敗する
- **AND** `Err(TickDriverError::HandleUnavailable)` が返される

#### Scenario: TokioTickDriver は auto_metadata を返す

- **GIVEN** `TokioTickDriver::default()` が生成される
- **WHEN** `provision` が成功する
- **THEN** `auto_metadata` は `Some(AutoDriverMetadata { profile: AutoProfileKind::Tokio, ... })` を含む
- **AND** `driver_id` と `resolution` が正しく設定される

#### Scenario: TokioTickDriverStopper は全タスクの完了を待って停止する

- **GIVEN** `TokioTickDriver` が provision 済みで 2 つの async task が稼働中
- **WHEN** `stopper.stop()` が呼ばれる
- **THEN** `AtomicBool` 停止フラグが false に設定される
- **AND** 両 async task がフラグを検知してループを抜ける
- **AND** `std::sync::mpsc::Receiver` で両タスクの完了通知を受信して返る
- **AND** `stop` が返った時点で両 task は完全に停止済み
- **AND** 返却後に feed / executor へのアクセスは一切発生しない

#### Scenario: 旧 Tokio 実装が存在しない

- **GIVEN** 本 change が適用された状態
- **WHEN** 旧 `TokioTickDriver`（旧 `TickDriver` trait 実装）、`TokioTickExecutorPump`、`TokioTickDriverControl`、`TokioTickExecutorControl`、`default_tick_driver_config()`、`tick_driver_config_with_resolution()` を参照するコードをコンパイルする
- **THEN** コンパイルエラーになる（型・関数が存在しない）

### Requirement: 新 TickDriver trait 用のテスト driver を提供する

旧 `ManualTestDriver` を削除し、新 `TickDriver` trait 用のテスト driver で置き換えなければならない（MUST）。テスト driver は `runner_api_enabled` の自動有効化パスを新 API 側に実装しなければならない（MUST）。

#### Scenario: テスト driver で ActorSystem を起動できる

- **GIVEN** 新テスト driver が生成される
- **WHEN** `ActorSystemConfig::new(test_driver)` + `create_with_config` でシステムを起動する
- **THEN** システムが正常に起動する
- **AND** テスト用の制御 API（tick 手動進行等）が利用可能

#### Scenario: テスト driver は runner_api_enabled を自動有効化する

- **GIVEN** テスト driver を使用してシステムを起動する
- **WHEN** bootstrap が実行される
- **THEN** `runner_api_enabled` が自動的に有効化される
- **AND** 旧 `ManualTestDriver` の special path と同等の機能が提供される

#### Scenario: 旧 ManualTestDriver が存在しない

- **GIVEN** 本 change が適用された状態
- **WHEN** `ManualTestDriver` を参照するコードをコンパイルする
- **THEN** コンパイルエラーになる（型が存在しない）

### Requirement: TickDriverKind は non_exhaustive で Std と Tokio variant を持つ

`TickDriverKind` に `#[non_exhaustive]` を付与し、`Std` と `Tokio` variant を追加しなければならない（MUST）。これにより下流 crate が新 variant 追加時に壊れない。

#### Scenario: TickDriverKind に Std と Tokio が含まれる

- **GIVEN** 本 change が適用された状態
- **WHEN** `TickDriverKind` の variant を列挙する
- **THEN** `Auto`, `Manual`, `Std`, `Tokio` の 4 variant が存在する
- **AND** `#[non_exhaustive]` により `match` 文にワイルドカードアームが必須となる
