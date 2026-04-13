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

tick driver は `ActorSystemConfig::with_new_tick_driver(impl TickDriver + 'static)` で config に格納しなければならない（MUST）。tick driver だけ別引数にしてはならない（MUST NOT）。旧 `with_tick_driver(TickDriverConfig)` の名前・シグネチャは Phase 1 で変更してはならない（MUST NOT）。

#### Scenario: config builder で tick driver を設定する

- **WHEN** `ActorSystemConfig::default().with_new_tick_driver(StdTickDriver::default())` が呼ばれる
- **THEN** config 内に `Box<dyn TickDriver>` として格納される
- **AND** `create_with_config(props, config)` で config を消費し、内部で `.take()` して driver を move できる

### Requirement: ActorSystemSetup も新 tick driver を受け付ける

`ActorSystemSetup` にも新 `TickDriver` を設定するメソッドを提供しなければならない（MUST）。Pekko 互換の setup 経路が旧設計に取り残されてはならない。

#### Scenario: setup facade で新 tick driver を設定する

- **WHEN** `ActorSystemSetup::default().with_new_tick_driver(StdTickDriver::default())` が呼ばれる
- **THEN** 内部の `ActorSystemConfig` に `Box<dyn TickDriver>` として格納される
- **AND** `ActorSystem::create_with_setup(props, setup)` で setup を消費してシステムが起動する

### Requirement: 旧 API は Phase 1 で残す

Phase 1 では旧 `ActorSystem::new(props, TickDriverConfig)`, `new_with_config(&props, &config)`, `new_with_setup(&props, &setup)` を削除してはならない（MUST NOT）。新 API は `create_with_config` / `create_with_setup` として並行追加する。

#### Scenario: 旧 API のテストが Phase 1 でそのまま通る

- **GIVEN** Phase 1 の変更が適用された状態
- **WHEN** `cargo check --tests --workspace` を実行する
- **THEN** 旧 API を使う既存テストが全てコンパイル・通過する

### Requirement: actor-adaptor-std は StdTickDriver を提供する

`actor-adaptor-std` は `std::thread` + `sleep` ベースの `TickDriver` 実装を提供しなければならない（MUST）。`TickPulseSource` / `HardwareTickDriver` の unsafe C ABI callback 機構を経由してはならない（MUST NOT）。

#### Scenario: StdTickDriver はデフォルト 10ms 解像度で動作する

- **GIVEN** `StdTickDriver::default()` が生成される
- **WHEN** `provision(self: Box<Self>, feed, executor)` が呼ばれる
- **THEN** `std::thread::spawn` で tick 生成スレッドと executor 駆動スレッドが起動される
- **AND** `resolution` は `Duration::from_millis(10)` が返される
- **AND** `TickPulseSource::set_callback` は使用されない
