## Context

ActorSystem の Scheduler は「定期的な tick 生成」と「溜まったタスクの executor 駆動」を必要とする。これは本質的に 1 つの概念（scheduler の駆動源）であり、ユーザは environment（std / Tokio / embedded）を選ぶだけで済むべき。

## Goals / Non-Goals

**Goals:**
- tick 生成と executor 駆動を 1 つの `TickDriver` trait に統合
- `ActorSystemConfig` の builder パターンに統一的に組み込む
- `StdTickDriver` を `actor-adaptor-std` に提供
- テスト用 driver を新設

**Non-Goals:**
- `TokioTickDriver` の新 trait 移行（本 change では std adapter のみ。Tokio adapter は別 change）
- `TypedActorSystem::create_with_setup`（現行にも `new_with_setup` がないため不要。必要になった場合に別途検討）

## Decisions

### 1. 新 `TickDriver` trait — `self: Box<Self>` で object safety を確保

```rust
pub trait TickDriver: Send + 'static {
  /// Scheduler の駆動を開始する。
  ///
  /// `self: Box<Self>` で所有権を消費し、駆動結果を返す。
  /// provision 後に driver を再利用できないことがコンパイル時に保証される。
  ///
  /// このメソッドは即座に return する。内部でバックグラウンドの
  /// thread / async task を spawn し、それらが `resolution` 間隔で
  /// `feed` に tick を積み、`executor` で溜まったタスクを実行し続ける。
  /// バックグラウンド処理の停止は戻り値の `stopper` で行う。
  ///
  /// どう orchestrate するかは実装者の自由:
  /// - std: 2 thread + `thread::sleep` による timing 制御
  /// - Tokio: 2 async task + `tokio::time::interval` / `sleep`
  /// - embedded: ハードウェア割り込み + main loop
  fn provision(
    self: Box<Self>,
    feed: TickFeedHandle,
    executor: SchedulerTickExecutor,
  ) -> Result<TickDriverProvision, TickDriverError>;
}
```

`self: Box<Self>` にする理由:
- `ActorSystemConfig` は `Box<dyn TickDriver>` で型消去して格納する
- `provision(self, ...)` だと `where Self: Sized` が必要で object safety を壊す
- `self: Box<Self>` なら **object-safe かつ所有権消費**。1 メソッドで完結
- `provision_boxed` のような 2 メソッド構成は不要

トレードオフ: stack-allocated driver から直接 `provision` を呼べないが、呼ぶ場面がない（config が常に Box 化する）。

### 2. `TickDriverProvision` — snapshot 互換の戻り値

現行の bootstrap は provisioning 時に `TickDriverSnapshot` を生成し、`id` / `kind` / `resolution` / `auto_metadata` を event stream に publish する。新設計でもこの observability を維持する:

```rust
pub struct TickDriverProvision {
  /// Tick 解像度。
  pub resolution: Duration,
  /// Driver の一意識別子。
  pub id: TickDriverId,
  /// Driver の分類（observability 用）。
  pub kind: TickDriverKind,
  /// 駆動中の driver を停止するための制御オブジェクト。
  pub stopper: Box<dyn TickDriverStopper>,
  /// Auto driver metadata（Tokio 等の runtime 固有情報）。
  pub auto_metadata: Option<AutoDriverMetadata>,
}
```

bootstrap はこの戻り値から `TickDriverSnapshot` を構築して event stream に publish する。

### 3. `TickDriverStopper` — 所有権を取って join 可能な停止契約

旧 `TickDriverControl::shutdown(&self)` は atomic flag を倒すだけで thread join を待てない。新設計では停止時に所有権を消費する:

```rust
pub trait TickDriverStopper: Send + 'static {
  /// 停止を要求し、全スレッド/タスクの完了を待って返る。
  fn stop(self: Box<Self>);
}
```

`stop(self: Box<Self>)` で所有権を取るため、内部で `JoinHandle::join()` まで実行できる。旧 `TickDriverControl` は本 change で削除する。

### 4. 旧 `TickDriver` trait の置き換え

現行コードの `tick_driver_trait.rs` に旧 `TickDriver` trait が定義されている。本 change で旧 trait を削除し、新 trait で置き換える:

- `tick_driver_trait.rs` の旧 `TickDriver` trait を新 trait に置き換える
- `next_tick_driver_id()` は旧 trait と無関係な独立関数であるため、`tick_driver_id.rs` に移動する
- 旧 `TickDriverConfig` / `TickExecutorPump` / `HardwareTickDriver` / `TickPulseSource` / `ManualTestDriver` を削除する

### 5. 旧 API の削除と新 API への置き換え

旧 API を削除し、新 API に一本化する:

```
削除:
  ActorSystem::new(props, TickDriverConfig)
  ActorSystem::new_with_config(&props, &config)
  ActorSystem::new_with_config_and(&props, &config, f)
  ActorSystem::new_with_setup(&props, &setup)

新設:
  ActorSystem::create_with_config_and(props, config, f)  ← core メソッド、config を消費
  ActorSystem::create_with_config(props, config)         ← create_with_config_and に委譲
  ActorSystem::create_with_setup(props, setup)           ← create_with_config_and に委譲
```

showcase + テスト群も新 API に移行する。

### 6. `ActorSystemConfig` — `Option<Box<dyn TickDriver>>` で格納

```rust
pub struct ActorSystemConfig {
  tick_driver: Option<Box<dyn TickDriver>>,
  // ... 他のフィールドは変更なし
}

impl ActorSystemConfig {
  // コンストラクタ — TickDriver を必須引数にすることで推奨パスを明示
  // actor-core は no_std のためデフォルトの TickDriver を提供できない。
  // ユーザは environment adapter（StdTickDriver 等）を渡す。
  pub fn new(driver: impl TickDriver + 'static) -> Self {
    Self {
      tick_driver: Some(Box::new(driver)),
      ..Self::default()
    }
  }

  // driver を後から差し替える用途
  pub fn with_tick_driver(mut self, driver: impl TickDriver + 'static) -> Self {
    self.tick_driver = Some(Box::new(driver));
    self
  }
}
```

旧 `tick_driver_config: Option<TickDriverConfig>` フィールドと旧 `with_tick_driver(TickDriverConfig)` メソッドは削除する。`create_with_config` は config を消費（move）し、`tick_driver` を `.take()` して `provision` で消費する。

### 7. `ActorSystemSetup` — 新 API 対応

旧 `with_tick_driver(TickDriverConfig)` を削除し、新 API に置き換える:

```rust
impl ActorSystemSetup {
  pub fn with_tick_driver(self, driver: impl TickDriver + 'static) -> Self {
    Self { config: self.config.with_tick_driver(driver) }
  }

  // 消費して ActorSystemConfig を返す既存メソッド
  pub fn into_actor_system_config(self) -> ActorSystemConfig { self.config }
}
```

`ActorSystem::create_with_setup(props, setup)` を追加。内部で `setup.into_actor_system_config()` → `create_with_config` に委譲。

**`TypedActorSystem::create_with_setup` は本 change のスコープ外とする。** 現行 `TypedActorSystem` にも `new_with_setup` は存在しないため、`create_with_setup` も追加しない。必要になった場合は別途検討する。

### 8. bootstrap 新経路の統合

現行の `SystemState::build_from_config(config: &ActorSystemConfig)` は config を借用で受け取る。新 `create_with_config` は config を move で消費するため、旧メソッドを削除し新しい build 関数で置き換える:

```rust
// 新メソッド（config を move で受け取る）
pub(crate) fn build_from_owned_config(mut config: ActorSystemConfig) -> Result<Self, SpawnError> {
  // 新 tick driver を .take() で取り出す
  let driver = config.take_tick_driver();

  // 以下は build_from_config と同様の初期化処理...
  // driver が Some の場合: driver.provision(feed, executor) で起動
  // driver が None の場合: SpawnError::SystemBuildError

  // ManualTest 自動検出（#[cfg(any(test, feature = "test-support"))]）:
  // 新 API では新テスト driver を使うため、旧 ManualTest 検出ロジックは不要。
  // 新テスト driver は明示的に runner_api_enabled を有効化する API を持つ。
}
```

`ActorSystem::create_with_config` → `SystemState::build_from_owned_config` に委譲する。旧 `build_from_config(&ActorSystemConfig)` は削除する。

### 9. `create_with_config_and` — 拡張コールバック付き API

現行の `new_with_config_and<F>` は `configure: F` コールバックで extension 登録等を行う core メソッドであり、他の `new_*` メソッドが全てこれに委譲している。新 API でも同等の拡張点を提供する:

```rust
pub fn create_with_config_and<F>(
  user_guardian_props: &Props,
  config: ActorSystemConfig,
  configure: F,
) -> Result<Self, SpawnError>
where
  F: FnOnce(&ActorSystem) -> Result<(), SpawnError>,
{
  // SystemState::build_from_owned_config(config) で state を構築
  // configure(&system) でユーザコールバックを実行
}
```

`create_with_config` と `create_with_setup` はこのメソッドに委譲する。

### 10. テスト用 driver の新設

旧 `ManualTestDriver` を削除し、新 `TickDriver` trait 用のテスト driver で置き換える。`ManualTestDriver` 固有の special path（`build_from_config` 内の `runner_api_enabled` 自動有効化）も新 API 側で独立に実装する。

### 11. `StdTickDriver` — `std::thread` ベース

```rust
pub struct StdTickDriver {
  resolution: Duration,
}

impl StdTickDriver {
  pub fn new(resolution: Duration) -> Self {
    Self { resolution }
  }
}

impl Default for StdTickDriver {
  fn default() -> Self {
    Self { resolution: Duration::from_millis(10) }
  }
}

impl TickDriver for StdTickDriver {
  fn provision(
    self: Box<Self>,
    feed: TickFeedHandle,
    executor: SchedulerTickExecutor,
  ) -> Result<TickDriverProvision, TickDriverError> {
    let resolution = self.resolution;
    let id = next_tick_driver_id();

    // tick 生成 thread
    let tick_running = Arc::new(AtomicBool::new(true));
    let tick_flag = tick_running.clone();
    let tick_thread = thread::spawn(move || {
      while tick_flag.load(Ordering::Acquire) {
        thread::sleep(resolution);
        feed.enqueue(1);
      }
    });

    // executor 駆動 thread
    let exec_running = Arc::new(AtomicBool::new(true));
    let exec_flag = exec_running.clone();
    let exec_interval = resolution / 10;
    let exec_thread = thread::spawn(move || {
      while exec_flag.load(Ordering::Acquire) {
        executor.drive_pending();
        thread::sleep(exec_interval);
      }
    });

    Ok(TickDriverProvision {
      resolution,
      id,
      kind: TickDriverKind::Std,
      stopper: Box::new(StdTickDriverStopper {
        tick_running,
        tick_thread: Some(tick_thread),
        exec_running,
        exec_thread: Some(exec_thread),
      }),
      auto_metadata: None,
    })
  }
}

struct StdTickDriverStopper {
  tick_running: Arc<AtomicBool>,
  tick_thread:  Option<thread::JoinHandle<()>>,
  exec_running: Arc<AtomicBool>,
  exec_thread:  Option<thread::JoinHandle<()>>,
}

impl TickDriverStopper for StdTickDriverStopper {
  fn stop(mut self: Box<Self>) {
    self.tick_running.store(false, Ordering::Release);
    self.exec_running.store(false, Ordering::Release);
    if let Some(h) = self.tick_thread.take() {
      if h.join().is_err() {
        eprintln!("warn: tick driver tick thread panicked during shutdown");
      }
    }
    if let Some(h) = self.exec_thread.take() {
      if h.join().is_err() {
        eprintln!("warn: tick driver executor thread panicked during shutdown");
      }
    }
  }
}
```

## Risks / Trade-offs

- [Risk] `self: Box<Self>` により stack-allocated driver から直接 `provision` を呼べない → Mitigation: 呼ぶ場面がない。config が `with_tick_driver(impl TickDriver)` で常に Box 化する
- [Risk] `TickDriverKind` に `Std` variant を追加すると、`#[non_exhaustive]` でないため下流 crate の網羅的 `match` が壊れる → Mitigation: `TickDriverKind` に `#[non_exhaustive]` を付与してから variant を追加する。これ自体が破壊的変更だが、一度行えば以後の variant 追加は非破壊になる
- [Risk] `thread::sleep` の精度はプラットフォーム依存 → Mitigation: デフォルト 10ms は実用上十分。高精度が必要なら Tokio adapter を使用
- [Decision] `TickDriverError` は既存の variant をそのまま使う。新 `provision` メソッドの失敗は既存の `SpawnFailed` / `HandleUnavailable` でカバーできる。不足が判明した場合に variant を追加する
- [Decision] `StdTickDriverStopper::stop` のログ出力は `eprintln!` を使用する。`actor-adaptor-std` は `tracing` を必須依存に持たないため、std のみで完結させる

## Open Questions

- executor thread の駆動方式。現在の設計は `sleep(resolution / 10)` による polling だが、3 つの選択肢がある:
  - **A. sleep polling**: `sleep(resolution / 10)` → `drive_pending()` → ループ。CPU 負荷低だが最大 `resolution/10` の遅延
  - **B. yield busy loop**: `yield_now()` → `drive_pending()` → ループ。遅延最小だが CPU 100% 消費
  - **C. tick 駆動（通知ベース）**: tick thread が `feed.enqueue` 後に executor thread を `unpark` / `CondVar::notify` で起こす。executor thread は work がなければ `park` / `CondVar::wait` で休眠。CPU 負荷低かつ遅延最小だが、tick thread と executor thread の結合度が上がる
