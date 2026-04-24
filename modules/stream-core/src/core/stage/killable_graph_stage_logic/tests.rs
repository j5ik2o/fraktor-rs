use alloc::{boxed::Box, vec::Vec};

use fraktor_utils_core_rs::core::sync::{ArcShared, SpinSyncMutex};

use crate::core::{
  StreamError, UniqueKillSwitch,
  stage::{
    AsyncCallback, GraphStageLogic, KillableGraphStageLogic, StageActor, StageActorReceive, StageContext,
    TimerGraphStageLogic,
  },
};

// ---------------------------------------------------------------------------
// テスト用ミニマル StageContext 実装
// ---------------------------------------------------------------------------

/// 記録専用の `StageContext` 実装。
///
/// - `complete()` / `fail()` 呼び出しを記録し、Running 状態では呼ばれず、 Shutdown / Aborted
///   状態で必ず呼ばれることを検証するために使う。
/// - `pull` / `push` / `grab` は今回のテスト対象外（KillableGraphStageLogic
///   は入出力を直接操作しないため）。
struct TestCtx {
  async_cb:   AsyncCallback<u32>,
  timers:     TimerGraphStageLogic,
  completed:  bool,
  failed:     Option<StreamError>,
  pull_count: u32,
  pushes:     Vec<u32>,
  grab_value: u32,
}

impl TestCtx {
  fn new() -> Self {
    Self {
      async_cb:   AsyncCallback::new(),
      timers:     TimerGraphStageLogic::new(),
      completed:  false,
      failed:     None,
      pull_count: 0,
      pushes:     Vec::new(),
      grab_value: 0,
    }
  }
}

impl StageContext<u32, u32> for TestCtx {
  fn pull(&mut self) {
    self.pull_count += 1;
  }

  fn grab(&mut self) -> u32 {
    self.grab_value
  }

  fn push(&mut self, out: u32) {
    self.pushes.push(out);
  }

  fn complete(&mut self) {
    self.completed = true;
  }

  fn fail(&mut self, error: StreamError) {
    self.failed = Some(error);
  }

  fn async_callback(&self) -> &AsyncCallback<u32> {
    &self.async_cb
  }

  fn timer_graph_stage_logic(&mut self) -> &mut TimerGraphStageLogic {
    &mut self.timers
  }

  fn get_stage_actor(&mut self, receive: Box<dyn StageActorReceive>) -> Result<StageActor, StreamError> {
    drop(receive);
    Err(StreamError::ActorSystemMissing)
  }

  fn stage_actor(&self) -> Result<StageActor, StreamError> {
    Err(StreamError::StageActorRefNotInitialized)
  }
}

// ---------------------------------------------------------------------------
// テスト用 GraphStageLogic 実装
// ---------------------------------------------------------------------------

/// 各コールバック呼び出しを共有カウンタへ記録する。
///
/// `KillableGraphStageLogic` に inner を move した後でも外部から
/// 観測できるよう、カウンタは `ArcShared<SpinSyncMutex<_>>` で保持する。
#[derive(Default, Clone)]
struct Counters {
  on_start:          u32,
  on_pull:           u32,
  on_push:           u32,
  on_complete:       u32,
  on_error:          u32,
  last_error:        Option<StreamError>,
  on_async_callback: u32,
  on_timer:          u32,
  last_timer_key:    Option<u64>,
  on_stop:           u32,
}

impl Counters {
  /// いずれかのコールバックが呼ばれたか（Shutdown / Aborted 検証に使用）。
  fn any_callback_invoked(&self) -> bool {
    self.on_start > 0
      || self.on_pull > 0
      || self.on_push > 0
      || self.on_complete > 0
      || self.on_error > 0
      || self.on_async_callback > 0
      || self.on_timer > 0
      || self.on_stop > 0
  }
}

type SharedCounters = ArcShared<SpinSyncMutex<Counters>>;

fn new_shared_counters() -> SharedCounters {
  ArcShared::new(SpinSyncMutex::new(Counters::default()))
}

struct RecordingLogic {
  counters:  SharedCounters,
  mat_value: u32,
}

impl RecordingLogic {
  fn new(counters: SharedCounters, mat_value: u32) -> Self {
    Self { counters, mat_value }
  }
}

impl GraphStageLogic<u32, u32, u32> for RecordingLogic {
  fn on_start(&mut self, _ctx: &mut dyn StageContext<u32, u32>) {
    self.counters.lock().on_start += 1;
  }

  fn on_pull(&mut self, _ctx: &mut dyn StageContext<u32, u32>) {
    self.counters.lock().on_pull += 1;
  }

  fn on_push(&mut self, _ctx: &mut dyn StageContext<u32, u32>) {
    self.counters.lock().on_push += 1;
  }

  fn on_complete(&mut self, _ctx: &mut dyn StageContext<u32, u32>) {
    self.counters.lock().on_complete += 1;
  }

  fn on_error(&mut self, _ctx: &mut dyn StageContext<u32, u32>, error: StreamError) {
    let mut guard = self.counters.lock();
    guard.on_error += 1;
    guard.last_error = Some(error);
  }

  fn on_async_callback(&mut self, _ctx: &mut dyn StageContext<u32, u32>) {
    self.counters.lock().on_async_callback += 1;
  }

  fn on_timer(&mut self, _ctx: &mut dyn StageContext<u32, u32>, timer_key: u64) {
    let mut guard = self.counters.lock();
    guard.on_timer += 1;
    guard.last_timer_key = Some(timer_key);
  }

  fn on_stop(&mut self, _ctx: &mut dyn StageContext<u32, u32>) {
    self.counters.lock().on_stop += 1;
  }

  fn materialized(&mut self) -> u32 {
    self.mat_value
  }
}

// ---------------------------------------------------------------------------
// テスト用ヘルパー: 全 8 コールバックを順に呼ぶ
// ---------------------------------------------------------------------------

fn invoke_all_callbacks<L>(logic: &mut KillableGraphStageLogic<L, u32, u32, u32>, ctx: &mut TestCtx)
where
  L: GraphStageLogic<u32, u32, u32> + Send, {
  logic.on_start(ctx);
  logic.on_pull(ctx);
  logic.on_push(ctx);
  logic.on_complete(ctx);
  logic.on_error(ctx, StreamError::Failed);
  logic.on_async_callback(ctx);
  logic.on_timer(ctx, 42);
  logic.on_stop(ctx);
}

// ---------------------------------------------------------------------------
// コンストラクタ
// ---------------------------------------------------------------------------

#[test]
fn new_constructor_accepts_kill_state_handle() {
  // Given: UniqueKillSwitch から state_handle を取得
  let switch = UniqueKillSwitch::new();
  let counters = new_shared_counters();
  let inner = RecordingLogic::new(counters.clone(), 7);

  // When: `new(inner, state_handle)` で構築 → 1 回 on_pull を呼ぶ
  let mut logic: KillableGraphStageLogic<RecordingLogic, u32, u32, u32> =
    KillableGraphStageLogic::new(inner, switch.state_handle());
  let mut ctx = TestCtx::new();
  logic.on_pull(&mut ctx);

  // Then: Running のため inner に委譲され、カウンタが増える
  assert_eq!(counters.lock().on_pull, 1);
  assert!(!ctx.completed);
  assert!(ctx.failed.is_none());
}

#[test]
fn from_kill_switch_constructor_shares_state_with_switch() {
  // Given: UniqueKillSwitch と RecordingLogic
  let switch = UniqueKillSwitch::new();
  let counters = new_shared_counters();
  let inner = RecordingLogic::new(counters.clone(), 3);

  // When: `from_kill_switch(inner, &switch)` で構築 → switch.shutdown() を外部から発行
  let mut logic: KillableGraphStageLogic<RecordingLogic, u32, u32, u32> =
    KillableGraphStageLogic::from_kill_switch(inner, &switch);
  switch.shutdown();
  let mut ctx = TestCtx::new();
  logic.on_start(&mut ctx);

  // Then: switch の shutdown が logic 側にも反映され ctx.complete() が呼ばれる
  // （KillSwitchStateHandle を共有している証拠）
  assert!(ctx.completed);
  assert_eq!(counters.lock().on_start, 0);
}

// ---------------------------------------------------------------------------
// Running 状態: 各コールバックが inner にそのまま駆動される
// ---------------------------------------------------------------------------

#[test]
fn running_state_delegates_every_callback_to_inner() {
  // Given: Running 状態の KillableGraphStageLogic と TestCtx
  let switch = UniqueKillSwitch::new();
  let counters = new_shared_counters();
  let inner = RecordingLogic::new(counters.clone(), 0);
  let mut logic = KillableGraphStageLogic::from_kill_switch(inner, &switch);
  let mut ctx = TestCtx::new();

  // When: 全コールバックを 1 回ずつ呼ぶ
  invoke_all_callbacks(&mut logic, &mut ctx);

  // Then: inner の各カウンタが 1 ずつ増える（委譲された証拠）
  let snapshot = counters.lock().clone();
  assert_eq!(snapshot.on_start, 1);
  assert_eq!(snapshot.on_pull, 1);
  assert_eq!(snapshot.on_push, 1);
  assert_eq!(snapshot.on_complete, 1);
  assert_eq!(snapshot.on_error, 1);
  assert_eq!(snapshot.last_error, Some(StreamError::Failed));
  assert_eq!(snapshot.on_async_callback, 1);
  assert_eq!(snapshot.on_timer, 1);
  assert_eq!(snapshot.last_timer_key, Some(42));
  assert_eq!(snapshot.on_stop, 1);

  // Then: Running 状態では ctx.complete / ctx.fail は呼ばれていない
  assert!(!ctx.completed);
  assert!(ctx.failed.is_none());
}

// ---------------------------------------------------------------------------
// Shutdown 遷移: 次コールバックで ctx.complete() へ遷移し、inner は呼ばれない
// ---------------------------------------------------------------------------

#[test]
fn shutdown_transition_invokes_complete_and_skips_inner() {
  // Given: Running 状態で構築し、直後に switch.shutdown() を呼ぶ
  let switch = UniqueKillSwitch::new();
  let counters = new_shared_counters();
  let inner = RecordingLogic::new(counters.clone(), 0);
  let mut logic = KillableGraphStageLogic::from_kill_switch(inner, &switch);
  let mut ctx = TestCtx::new();
  switch.shutdown();

  // When: 全コールバックを 1 回ずつ呼ぶ
  invoke_all_callbacks(&mut logic, &mut ctx);

  // Then: 各コールバックで ctx.complete() が呼ばれたことを検証
  assert!(ctx.completed);
  assert!(ctx.failed.is_none());

  // Then: inner はいかなるコールバックも受け取っていない
  let snapshot = counters.lock().clone();
  assert!(!snapshot.any_callback_invoked(), "inner must not be driven after shutdown");
}

#[test]
fn shutdown_mid_stream_delegates_before_and_completes_after() {
  // Given: Running 状態の KillableGraphStageLogic
  let switch = UniqueKillSwitch::new();
  let counters = new_shared_counters();
  let inner = RecordingLogic::new(counters.clone(), 0);
  let mut logic = KillableGraphStageLogic::from_kill_switch(inner, &switch);
  let mut ctx = TestCtx::new();

  // When: 1 回 on_pull（Running のまま）→ shutdown() → 1 回 on_pull（Shutdown 後）
  logic.on_pull(&mut ctx);
  switch.shutdown();
  logic.on_pull(&mut ctx);

  // Then: 1 回目は inner に届き、2 回目は ctx.complete() へ遷移
  assert!(ctx.completed);
  assert!(ctx.failed.is_none());
  assert_eq!(counters.lock().on_pull, 1);
}

// ---------------------------------------------------------------------------
// Aborted 遷移: 次コールバックで ctx.fail(error) へ遷移し、inner は呼ばれない
// ---------------------------------------------------------------------------

#[test]
fn abort_transition_invokes_fail_with_error_and_skips_inner() {
  // Given: Running 状態で構築し、直後に switch.abort(error) を呼ぶ
  let switch = UniqueKillSwitch::new();
  let counters = new_shared_counters();
  let inner = RecordingLogic::new(counters.clone(), 0);
  let mut logic = KillableGraphStageLogic::from_kill_switch(inner, &switch);
  let mut ctx = TestCtx::new();
  switch.abort(StreamError::Failed);

  // When: 全コールバックを 1 回ずつ呼ぶ
  invoke_all_callbacks(&mut logic, &mut ctx);

  // Then: 各コールバックで ctx.fail(error) が呼ばれ、元のエラーが伝播している
  assert_eq!(ctx.failed, Some(StreamError::Failed));
  assert!(!ctx.completed);

  // Then: inner はいかなるコールバックも受け取っていない
  let snapshot = counters.lock().clone();
  assert!(!snapshot.any_callback_invoked(), "inner must not be driven after abort");
}

#[test]
fn abort_mid_stream_delegates_before_and_fails_after() {
  // Given: Running 状態の KillableGraphStageLogic
  let switch = UniqueKillSwitch::new();
  let counters = new_shared_counters();
  let inner = RecordingLogic::new(counters.clone(), 0);
  let mut logic = KillableGraphStageLogic::from_kill_switch(inner, &switch);
  let mut ctx = TestCtx::new();

  // When: 1 回 on_push（Running のまま）→ abort(StreamError::BufferOverflow) → 1 回 on_push
  logic.on_push(&mut ctx);
  switch.abort(StreamError::BufferOverflow);
  logic.on_push(&mut ctx);

  // Then: 1 回目は inner に届き、2 回目は ctx.fail(error) へ遷移（元のエラーを保持）
  assert_eq!(ctx.failed, Some(StreamError::BufferOverflow));
  assert!(!ctx.completed);
  assert_eq!(counters.lock().on_push, 1);
}

// ---------------------------------------------------------------------------
// materialized() が inner に委譲され Mat を正しく返す
// ---------------------------------------------------------------------------

#[test]
fn materialized_delegates_to_inner_and_returns_mat_value() {
  // Given: inner が `mat_value: 123` を保持
  let switch = UniqueKillSwitch::new();
  let counters = new_shared_counters();
  let inner = RecordingLogic::new(counters, 123);
  let mut logic = KillableGraphStageLogic::from_kill_switch(inner, &switch);

  // When: materialized() を呼ぶ
  let mat = logic.materialized();

  // Then: inner の mat_value がそのまま返される
  assert_eq!(mat, 123);
}

#[test]
fn materialized_is_unaffected_by_kill_state() {
  // Given: shutdown / abort 済みでも materialized() は state に依存しない
  let switch = UniqueKillSwitch::new();
  let counters = new_shared_counters();
  let inner = RecordingLogic::new(counters, 99);
  let mut logic = KillableGraphStageLogic::from_kill_switch(inner, &switch);
  switch.shutdown();

  // When: materialized() を呼ぶ
  let mat = logic.materialized();

  // Then: kill 状態に関係なく inner の mat_value が返る
  assert_eq!(mat, 99);
}
