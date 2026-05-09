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

use fraktor_actor_core_kernel_rs::pattern::Clock;

use super::OptimalSizeExploringResizer;
use crate::dsl::routing::Resizer;

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

// ai-review-batch4-001 の回帰テスト:
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
  // → Pekko の Double.toInt 相当で raw target = (1 * 0.8).toInt = 0
  // → proposed delta = 0 - 5 = -5 だが lower_bound=1 への clamp で 5 + (-5) → 1 になり、
  //    最終 delta = 1 - 5 = -4
  let delta = resizer.resize(&[0; 5]);
  assert_eq!(delta, -4, "未活用連続期間経過で lower_bound=1 までクランプされ delta=-4");
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
  // currentSize=10 の window = [left=min(<10,adjacency).take(8).last→unset→10,
  //                             right=max(>=10,adjacency).take(8).last→14]
  // performance_log: {10→100ms, 12→200ms, 14→50ms}
  // 左側には 10 未満のサイズがないため left_boundary は fallback の currentSize=10
  // 右側候補 {10, 12, 14} から adjacency 順に全て取って右境界=14
  // window = [10, 14] に絞り込み、最速 = 14 (50ms)
  // movement = (14 - 10) / 2 = 2.0 → ceil(2.0) = 2
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

// Pekko `OptimalSizeExploringResizer.scala:293-296` の境界フィルタは
// 左を `filter(_ < currentSize)`（strict less-than）、右を `filter(_ >= currentSize)`
// と非対称に定義している。そのため `current_size` が `performance_log` に含まれる
// ケース（`report_message_count` が毎 tick で現サイズを記録する運用下では common case）
// で、`current_size` は右境界の候補にのみ数えられ、右の枠を 1 つ消費する。
//
// 本テストは境界フィルタの `<` / `>=` 非対称性を fraktor-rs 側で固定し、Pekko との
// 互換が将来的にドリフトしないよう garde する。
#[test]
fn optimize_window_matches_pekko_boundary_asymmetry() {
  let clock = FakeClock::new();
  // optimization_range = 4 → num_each_side = 2
  let resizer =
    OptimalSizeExploringResizer::new(1, 30, clock, SEED).with_exploration_probability(0.0).with_optimization_range(4);

  // performance_log: {8, 10, 11, 12, 15}、current_size = 10
  //
  // Pekko 境界計算:
  //   lower = filter(< 10) = {8}、adjacency 順 [8]、take(2).last -> 8
  //   upper = filter(>= 10) = {10, 11, 12, 15}、adjacency 順 [10, 11, 12, 15]、
  //                take(2).last -> 11  ← `current_size` が右枠を 1 つ消費
  //   window = [8, 11] のみで 12 と 15 は除外される
  //
  // 速度は 12 を最速に設定しているが window 外なので、window 内の最速 11 (30ms) が勝つ。
  // もし左境界を `<=` に変えると lower = {8, 10} → [10, 8] → last=8 で同じだが、
  // 右境界は変わらず 11。差分を効かせるには右側で `current_size` を含めない実装に
  // 変えた場合を考える必要があるが、Pekko 準拠であれば下記の期待値が成立する。
  //
  // movement = (11 - 10) / 2 = 0.5 → ceil(0.5) = 1
  {
    let mut guard = resizer.state.lock();
    guard.performance_log = BTreeMap::from([
      (8_usize, Duration::from_millis(100)),
      (10_usize, Duration::from_millis(50)),
      (11_usize, Duration::from_millis(30)),
      (12_usize, Duration::from_millis(10)), // window 外（より速いが採用されない）
      (15_usize, Duration::from_millis(5)),  // window 外
    ]);
  }

  let delta = resizer.resize(&[0; 10]);
  assert_eq!(
    delta, 1,
    "Pekko 境界 (< / >=) で window=[8,11] となり window 外の 12/15 は無視、window 内最速 11 へ半歩移動",
  );
}

// 右境界の `filter(_ >= current_size)` によって `current_size` 自身が `num_each_side`
// の 1 枠を消費することを、より狭い window で直接観測する回帰テスト。
//
// optimization_range = 2 → num_each_side = 1。右側候補が `{current, current+1, ...}`
// のとき、adjacency ソートで先頭は `current` なので take(1).last = `current`。つまり
// 右境界は自身に閉じ、`current+1` 以上は window 外となる。
#[test]
fn optimize_right_boundary_closes_at_current_size_when_num_each_side_is_one() {
  let clock = FakeClock::new();
  let resizer =
    OptimalSizeExploringResizer::new(1, 30, clock, SEED).with_exploration_probability(0.0).with_optimization_range(2);

  // performance_log: {9, 10, 11}、current_size = 10
  // lower = filter(< 10) = {9}、take(1).last -> 9
  // upper = filter(>= 10) = {10, 11}、adjacency 順 [10, 11]、take(1).last -> 10
  // window = [9, 10]。11 は除外。
  // 速度: {9→10ms (fastest), 10→50ms, 11→1ms (window 外)}
  // window 内最速 = 9 (10ms)
  // movement = (9 - 10) / 2.0 = -0.5 → floor(-0.5) = -1
  {
    let mut guard = resizer.state.lock();
    guard.performance_log = BTreeMap::from([
      (9_usize, Duration::from_millis(10)),
      (10_usize, Duration::from_millis(50)),
      (11_usize, Duration::from_millis(1)), // window 外
    ]);
  }

  let delta = resizer.resize(&[0; 10]);
  assert_eq!(delta, -1, "右境界は >= フィルタで current_size に閉じ、11 は window 外として除外される");
}
