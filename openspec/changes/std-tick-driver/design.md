## Context

ActorSystem の Scheduler は「定期的な tick 生成」と「溜まったタスクの executor 駆動」を必要とする。これは本質的に 1 つの概念（scheduler の駆動源）であり、ユーザは environment（std / Tokio / embedded）を選ぶだけで済むべき。

## Goals / Non-Goals

**Goals:**
- tick 生成と executor 駆動を 1 つの `TickDriver` trait に統合
- `ActorSystemConfig` の builder パターンに統一的に組み込む
- `StdTickDriver` を `actor-adaptor-std` に提供
- テスト用 driver を新設

**Non-Goals:**
- 旧 trait / enum の削除（Phase 3）
- showcase の移行（Phase 2）
- `TokioTickDriver` の移行（Phase 2）

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

`stop(self: Box<Self>)` で所有権を取るため、内部で `JoinHandle::join()` まで実行できる。旧 `TickDriverControl` は触らない。

### 4. ストラングラーフィグ — 旧 API と並行して新 API を追加

Phase 1 では旧 API を残したまま新 API を追加する。テスト群は旧 API のまま。

**旧 `new_with_config` のシグネチャは変更しない。** 新 `create_with_config` を別メソッドとして追加する。

```
Phase 1 (本 change):
  旧: ActorSystem::new(props, TickDriverConfig)           ← Phase 1 では残す、Phase 3 で削除
  旧: ActorSystem::new_with_config(&props, &config)       ← Phase 1 では残す、Phase 3 で削除
  旧: ActorSystem::new_with_setup(&props, &setup)         ← Phase 1 では残す、Phase 3 で削除
  新: ActorSystem::create_with_config(props, config)      ← 追加（config を消費）

Phase 2 (別 change):
  テスト群 + showcase を新 API に移行
  旧 API を deprecated 化

Phase 3 (別 change):
  旧 API / TickDriverConfig / TickExecutorPump / ManualTestDriver を削除
```

### 5. `ActorSystemConfig` — `Option<Box<dyn TickDriver>>` で格納

```rust
pub struct ActorSystemConfig {
  // 旧フィールド（Phase 1 では残す、Phase 3 で削除）
  tick_driver_config: Option<TickDriverConfig>,
  // 新フィールド
  new_tick_driver: Option<Box<dyn TickDriver>>,
  // ... 他のフィールドは変更なし
}

impl ActorSystemConfig {
  // 旧メソッド（名前・シグネチャを変更しない）
  pub fn with_tick_driver(mut self, config: TickDriverConfig) -> Self {
    self.tick_driver_config = Some(config);
    self
  }

  // 新メソッド（Phase 3 で旧を削除後、with_tick_driver に改名）
  pub fn with_new_tick_driver(mut self, driver: impl TickDriver + 'static) -> Self {
    self.new_tick_driver = Some(Box::new(driver));
    self
  }
}
```

`create_with_config` は config を消費（move）し、`new_tick_driver` を `.take()` して `provision` で消費する。旧 `new_with_config` は `&config` のまま旧 `tick_driver_config` を使う。

**新旧フィールドの優先ルール**: `create_with_config` は新 `new_tick_driver` フィールドのみを参照する。旧 `tick_driver_config` は無視する。旧 `new_with_config` は旧 `tick_driver_config` のみを参照する。新旧が混在する状態（両方 `Some`）にはならない想定だが、万一両方セットされた場合は各 API が自分のフィールドしか見ない。

### 6. `ActorSystemSetup` — 新 API 対応

`ActorSystemSetup` も同様に新メソッドを追加:

```rust
impl ActorSystemSetup {
  // 旧メソッド（残す）
  pub fn with_tick_driver(self, config: TickDriverConfig) -> Self { ... }

  // 新メソッド
  pub fn with_new_tick_driver(self, driver: impl TickDriver + 'static) -> Self {
    Self { config: self.config.with_tick_driver(driver) }
  }

  // 消費して ActorSystemConfig を返す既存メソッド
  pub fn into_actor_system_config(self) -> ActorSystemConfig { self.config }
}
```

`ActorSystem::create_with_setup(props, setup)` を追加。内部で `setup.into_actor_system_config()` → `create_with_config` に委譲。

Phase 3 で旧 `with_tick_driver(TickDriverConfig)` を削除した後、`with_new_tick_driver` を `with_tick_driver` に改名する。

### 7. テスト用 driver の新設

旧 `ManualTestDriver` は触らない。新 `TickDriver` trait 用のテスト driver を新しく作る。`ManualTestDriver` 固有の special path（`build_from_config` 内の `runner_api_enabled` 自動有効化）も新 API 側で独立に実装する。

### 8. `StdTickDriver` — `std::thread` ベース

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
- [Risk] `TickDriverKind` に `Std` variant を追加すると、`#[non_exhaustive]` でないため下流 crate の網羅的 `match` が壊れる → Mitigation: `TickDriverKind` に `#[non_exhaustive]` を付与してから variant を追加する。これ自体が破壊的変更だが、Phase 1 で一度だけ行えば以後の variant 追加は非破壊になる
- [Risk] `thread::sleep` の精度はプラットフォーム依存 → Mitigation: デフォルト 10ms は実用上十分。高精度が必要なら Tokio adapter を使用

## Open Questions

- executor thread の駆動方式。現在の設計は `sleep(resolution / 10)` による polling だが、3 つの選択肢がある:
  - **A. sleep polling**: `sleep(resolution / 10)` → `drive_pending()` → ループ。CPU 負荷低だが最大 `resolution/10` の遅延
  - **B. yield busy loop**: `yield_now()` → `drive_pending()` → ループ。遅延最小だが CPU 100% 消費
  - **C. tick 駆動（通知ベース）**: tick thread が `feed.enqueue` 後に executor thread を `unpark` / `CondVar::notify` で起こす。executor thread は work がなければ `park` / `CondVar::wait` で休眠。CPU 負荷低かつ遅延最小だが、tick thread と executor thread の結合度が上がる
