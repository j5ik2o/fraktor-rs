//! Tests for `OptimalSizeExploringResizer`.
//!
//! Test strategy (plan.4 §6.9):
//! - 1..5:  parameter validation (`#[should_panic]`)
//! - 6:     contract 2 — `action_interval`-based resize gating
//! - 7..8:  contract 3 — only fully-utilized samples update `performance_log`
//! - 9:     contract 4 — downsize after the under-utilized period elapses
//! - 10..11: contract 5 — bounds clamping (`lower_bound` / `upper_bound`)
//! - 12:    short-circuit path (empty perf_log + no under-utilization)
//! - 13..14: explore probability branches
//! - 15:    optimize half-step movement

use alloc::{collections::BTreeMap, sync::Arc};
use core::{
  sync::atomic::{AtomicU64, Ordering},
  time::Duration,
};

use super::OptimalSizeExploringResizer;
use crate::core::{kernel::pattern::Clock, typed::dsl::routing::Resizer};

// テスト専用の決定論的時計（`circuit_breaker/tests.rs` と同一のパターン）。
#[derive(Clone)]
struct FakeClock {
  offset_millis: Arc<AtomicU64>,
}

impl FakeClock {
  fn new() -> Self {
    Self { offset_millis: Arc::new(AtomicU64::new(0)) }
  }

  fn advance(&self, duration: Duration) {
    self.offset_millis.fetch_add(duration.as_millis() as u64, Ordering::SeqCst);
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FakeInstant(u64);

impl Clock for FakeClock {
  type Instant = FakeInstant;

  fn now(&self) -> Self::Instant {
    FakeInstant(self.offset_millis.load(Ordering::SeqCst))
  }

  fn elapsed_since(&self, earlier: Self::Instant) -> Duration {
    let now = self.offset_millis.load(Ordering::SeqCst);
    Duration::from_millis(now.saturating_sub(earlier.0))
  }
}

// 全テスト共通の決定論的 RNG シード。
const SEED: u64 = 42;

// ==========================================================================
// 1. パラメータバリデーション（Pekko `IllegalArgumentException` の Rust 翻訳）
// ==========================================================================

#[test]
#[should_panic(expected = "lower_bound must be positive")]
fn new_rejects_zero_lower_bound() {
  let _ = OptimalSizeExploringResizer::new(0, 30, FakeClock::new(), SEED);
}

#[test]
#[should_panic(expected = "upper_bound must be >= lower_bound")]
fn new_rejects_upper_below_lower() {
  let _ = OptimalSizeExploringResizer::new(10, 5, FakeClock::new(), SEED);
}

#[test]
#[should_panic(expected = "exploration_probability must be in [0.0, 1.0]")]
fn with_exploration_probability_rejects_out_of_range() {
  let _ = OptimalSizeExploringResizer::new(1, 30, FakeClock::new(), SEED).with_exploration_probability(1.5);
}

#[test]
#[should_panic(expected = "optimization_range must be >= 2")]
fn with_optimization_range_rejects_below_two() {
  let _ = OptimalSizeExploringResizer::new(1, 30, FakeClock::new(), SEED).with_optimization_range(1);
}

#[test]
#[should_panic(expected = "weight_of_latest_metric must be in [0.0, 1.0]")]
fn with_weight_of_latest_metric_rejects_out_of_range() {
  let _ = OptimalSizeExploringResizer::new(1, 30, FakeClock::new(), SEED).with_weight_of_latest_metric(-0.1);
}

// ==========================================================================
// 2. 契約 2: `actionInterval` 経過ベースのリサイズ判定
// ==========================================================================

#[test]
fn is_time_for_resize_respects_action_interval() {
  let clock = FakeClock::new();
  let resizer =
    OptimalSizeExploringResizer::new(1, 30, clock.clone(), SEED).with_action_interval(Duration::from_millis(100));

  // action_interval=100ms 未経過 → false
  clock.advance(Duration::from_millis(50));
  assert!(!resizer.is_time_for_resize(0), "action_interval(100ms) 未経過時は is_time_for_resize = false");

  // さらに 100ms 進めて合計 150ms 経過 → true
  clock.advance(Duration::from_millis(100));
  assert!(resizer.is_time_for_resize(0), "action_interval(100ms) 経過後は is_time_for_resize = true");
}

// ==========================================================================
// 3. 契約 3: フル稼働時のみ performance_log を更新
// ==========================================================================

#[test]
fn report_message_count_ignores_non_fully_utilized_samples() {
  let clock = FakeClock::new();
  let resizer = OptimalSizeExploringResizer::new(1, 30, clock, SEED);

  // routee[1] が空 mailbox (size=0) → 非フル稼働
  resizer.report_message_count(&[1, 0, 1], 10);

  let guard = resizer.state.lock();
  assert!(guard.performance_log.is_empty(), "非フル稼働サンプルでは performance_log を更新してはならない");
  assert!(guard.record.under_utilization_streak.is_some(), "非フル稼働時は under_utilization_streak を記録すること");
}

#[test]
fn report_message_count_updates_performance_log_on_fully_utilized() {
  let clock = FakeClock::new();
  let resizer = OptimalSizeExploringResizer::new(1, 30, clock.clone(), SEED);

  // 1 回目: baseline 確立（has_recorded=false ゲートにより perf_log は未更新）
  resizer.report_message_count(&[1, 1, 1], 10);

  // 100ms 経過後の 2 回目フル稼働サンプル
  clock.advance(Duration::from_millis(100));
  resizer.report_message_count(&[1, 1, 1], 50);

  let guard = resizer.state.lock();
  assert!(
    guard.performance_log.contains_key(&3),
    "フル稼働サンプルが 2 回続いたら currentSize=3 の performance_log が更新されること"
  );
}

// Regression for ai-review-batch4-001:
// サンプル間にキューが拡大（`queue_size_change` が負）し、受信メッセージ数で
// それを相殺できない場合、Pekko 原典は `totalProcessed <= 0` で perf_log 更新を
// スキップする。Rust 側の `saturating_sub` 実装では `queue_size_change` が 0 に
// クランプされ、受信メッセージ数が誤って perf_log に記録されてしまっていた。
#[test]
fn report_message_count_skips_perf_log_when_queue_grew_faster_than_intake() {
  let clock = FakeClock::new();
  let resizer = OptimalSizeExploringResizer::new(1, 30, clock.clone(), SEED);

  // 1 回目: baseline（prior_queue = 5*3 = 15, message_count = 0）
  resizer.report_message_count(&[5, 5, 5], 0);

  // 2 回目: キューが拡大（20*3 = 60）、受信メッセージは 5 のみ。
  // queue_size_change = 15 - 60 = -45、total_processed = -45 + 5 = -40 <= 0
  clock.advance(Duration::from_millis(100));
  resizer.report_message_count(&[20, 20, 20], 5);

  let guard = resizer.state.lock();
  assert!(
    !guard.performance_log.contains_key(&3),
    "キュー流入がメッセージ処理より速かったサンプルは perf_log に記録してはならない"
  );
}

// ==========================================================================
// 4. 契約 4: `downsizeAfterUnderutilizedFor` 経過後の縮小
// ==========================================================================

#[test]
fn resize_downsizes_after_underutilized_period() {
  let clock = FakeClock::new();
  let resizer = OptimalSizeExploringResizer::new(1, 30, clock.clone(), SEED)
    .with_downsize_after_underutilized_for(Duration::from_millis(1000))
    .with_downsize_ratio(0.8);

  // currentSize=1 の非フル稼働サンプルで under_utilization_streak を開始
  // （highest_utilization=1 が streak に記録される）
  resizer.report_message_count(&[0], 10);

  // 未活用連続期間を 1500ms 経過（downsize_after_underutilized_for=1000ms を超過）
  clock.advance(Duration::from_millis(1500));

  // currentSize=5, highest_utilization=1, downsize_ratio=0.8
  // → target = ceil(1 * 0.8) = 1, delta = 1 - 5 = -4
  // 境界 [1,30] 内で維持
  let delta = resizer.resize(&[0; 5]);
  assert_eq!(delta, -4, "未活用連続期間経過で highest_utilization(1)*downsize_ratio(0.8)=1 へ縮小 (delta=-4)");
}

// ==========================================================================
// 5. 契約 5: 境界遵守（lower_bound / upper_bound クランプ）
// ==========================================================================

#[test]
fn resize_respects_lower_bound() {
  let clock = FakeClock::new();
  let resizer = OptimalSizeExploringResizer::new(13, 30, clock, SEED);

  // performance_log 空 + 未活用ストリークなし → proposedSize = 12 + 0 = 12
  // 12 < lower_bound(13) → return 13 - 12 = 1
  let delta = resizer.resize(&[0; 12]);
  assert_eq!(delta, 1, "lower_bound(13) 未満の currentSize(12) は lower_bound へ引き上げ (delta=1)");
}

#[test]
fn resize_respects_upper_bound() {
  let clock = FakeClock::new();
  let resizer = OptimalSizeExploringResizer::new(1, 20, clock, SEED);

  // performance_log 空 + 未活用ストリークなし → proposedSize = 25 + 0 = 25
  // 25 > upper_bound(20) → return 20 - 25 = -5
  let delta = resizer.resize(&[0; 25]);
  assert_eq!(delta, -5, "upper_bound(20) 超過の currentSize(25) は upper_bound へ引き下げ (delta=-5)");
}

// ==========================================================================
// 6. short-circuit: performance_log 空かつ未活用ストリークなし
// ==========================================================================

#[test]
fn resize_noop_when_performance_log_empty_and_not_underutilized() {
  let clock = FakeClock::new();
  let resizer = OptimalSizeExploringResizer::new(1, 20, clock, SEED);

  // 初期状態: performance_log 空 + under_utilization_streak=None
  // currentSize=10, delta=0, proposedSize=10, 境界 [1,20] 内 → 0
  let delta = resizer.resize(&[0; 10]);
  assert_eq!(delta, 0, "performance_log 空 + 境界内 → 変更なし (delta=0)");
}

// ==========================================================================
// 7. 確率分岐: explore / optimize
// ==========================================================================

#[test]
fn explore_stays_positive_most_of_the_time() {
  let clock = FakeClock::new();
  let resizer = OptimalSizeExploringResizer::new(1, 30, clock, SEED)
    .with_exploration_probability(1.0)
    .with_chance_of_scaling_down_when_full(0.0);

  // performance_log を非空に priming して short-circuit を回避
  {
    let mut guard = resizer.state.lock();
    guard.performance_log = BTreeMap::from([(10_usize, Duration::from_millis(100))]);
  }

  // exploration_probability=1.0 → 毎回 explore 分岐
  // chance_of_scaling_down_when_full=0.0 → rand < 0.0 は恒偽 → 常に +change
  // explore の change = max(1, rand.nextInt(ceil(10 * 0.1))) = max(1, rand.nextInt(1)) = 1
  for iteration in 0..20 {
    let delta = resizer.resize(&[0; 10]);
    assert!(
      delta >= 1,
      "chance_of_scaling_down_when_full=0.0 では explore は正方向のみ (iter={iteration}, delta={delta})"
    );
  }
}

#[test]
fn explore_goes_negative_when_chance_is_one() {
  let clock = FakeClock::new();
  let resizer = OptimalSizeExploringResizer::new(1, 30, clock, SEED)
    .with_exploration_probability(1.0)
    .with_chance_of_scaling_down_when_full(1.0);

  // performance_log を非空に priming して short-circuit を回避
  {
    let mut guard = resizer.state.lock();
    guard.performance_log = BTreeMap::from([(10_usize, Duration::from_millis(100))]);
  }

  // exploration_probability=1.0 → 毎回 explore 分岐
  // chance_of_scaling_down_when_full=1.0 → rand < 1.0 は恒真（nextDouble∈[0,1)） → 常に -change
  for iteration in 0..20 {
    let delta = resizer.resize(&[0; 10]);
    assert!(
      delta <= -1,
      "chance_of_scaling_down_when_full=1.0 では explore は負方向のみ (iter={iteration}, delta={delta})"
    );
  }
}

#[test]
fn optimize_moves_half_way_toward_best_size() {
  let clock = FakeClock::new();
  let resizer = OptimalSizeExploringResizer::new(1, 30, clock, SEED).with_exploration_probability(0.0);

  // 既定 optimization_range=16 → numOfSizesEachSide=8
  // currentSize=10 の window = [10-8, 10+8] = [2, 18]、currentSize 自体は除外
  // performance_log: {10→100ms(除外), 12→200ms, 14→50ms}
  // 最速サイズ = 14 (50ms), movement = 14 - 10 = 4, ceil(4/2) = 2
  {
    let mut guard = resizer.state.lock();
    guard.performance_log = BTreeMap::from([
      (10_usize, Duration::from_millis(100)),
      (12_usize, Duration::from_millis(200)),
      (14_usize, Duration::from_millis(50)),
    ]);
  }

  let delta = resizer.resize(&[0; 10]);
  assert_eq!(delta, 2, "optimize は最速サイズ (14, 50ms) に向け ceil(movement/2)=ceil(4/2)=2 で半歩移動");
}
