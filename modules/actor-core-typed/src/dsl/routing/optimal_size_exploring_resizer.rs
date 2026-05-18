//! Throughput-optimizing pool resizer inspired by Pekko's
//! `OptimalSizeExploringResizer`.

use alloc::{collections::BTreeMap, vec::Vec};
use core::time::Duration;

use fraktor_actor_core_kernel_rs::pattern::Clock;
use fraktor_utils_core_rs::sync::SpinSyncMutex;

use super::resizer::Resizer;

#[cfg(test)]
#[path = "optimal_size_exploring_resizer_test.rs"]
mod tests;

// ---------------------------------------------------------------------------
// Private support types
// ---------------------------------------------------------------------------
//
// 以下の 4 型は Pekko `OptimalSizeExploringResizer` の内部補助型に対応する
// `pub(crate)` 専用実装である。それぞれ独立ファイルに分けると
// `optimal_size_exploring_resizer` モジュールがサブモジュールを持つことになり、
// `routing.rs` 側で `pub use optimal_size_exploring_resizer::OptimalSizeExploringResizer;`
// を書くと `no-parent-reexport` lint に抵触する。型サイズがいずれも数十行で
// 公開 API には寄与しないため、公開 API 一貫性を優先して本体ファイルに inline する。

// --- LCG (Numerical Recipes MMIX constants) -------------------------------

/// Linear congruential generator with 64-bit state.
///
/// Replaces Pekko's `ThreadLocalRandom` in the `OptimalSizeExploringResizer`
/// algorithm so that explore / optimize branching is deterministic under a
/// fixed seed.
pub(crate) struct Lcg {
  state: u64,
}

impl Lcg {
  /// Creates a new generator seeded with `seed`.
  pub(crate) const fn new(seed: u64) -> Self {
    Self { state: seed }
  }

  /// Advances the state and returns the raw 64-bit value.
  const fn next_u64(&mut self) -> u64 {
    // Numerical Recipes MMIX constants.
    self.state = self.state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
    self.state
  }

  /// Returns a uniformly distributed value in `[0, 1)`.
  ///
  /// Uses the top 53 bits of the internal state, matching the precision of
  /// `f64` mantissa.
  pub(crate) fn next_f64(&mut self) -> f64 {
    let bits = self.next_u64() >> 11;
    let denom = (1_u64 << 53) as f64;
    bits as f64 / denom
  }

  /// Returns a uniformly distributed integer in `[0, bound)`.
  ///
  /// The caller must ensure `bound > 0`; passing zero yields an arithmetic
  /// panic, matching Pekko's `Random.nextInt(0)` (which throws
  /// `IllegalArgumentException`). Uses the high 32 bits of `next_u64` to avoid
  /// the well-known low-bit correlation weakness of an LCG, matching the
  /// high-bit extraction done by [`next_f64`](Self::next_f64).
  pub(crate) const fn next_u32_bounded(&mut self, bound: u32) -> u32 {
    ((self.next_u64() >> 32) as u32) % bound
  }
}

// --- Under-utilization streak ---------------------------------------------

/// Tracks a contiguous period during which the pool was not fully utilized.
///
/// Corresponds to Pekko's `OptimalSizeExploringResizer.UnderUtilizationStreak`.
/// The streak is reset (set back to `None`) whenever the pool becomes fully
/// utilized, and extended otherwise.
#[derive(Debug, Clone, Copy)]
pub(crate) struct UnderUtilizationStreak<I> {
  /// Timestamp when the streak started.
  pub(crate) start:               I,
  /// Highest number of busy routees observed during the streak.
  pub(crate) highest_utilization: usize,
}

// --- Resize record --------------------------------------------------------

/// Snapshot of router statistics retained between `report_message_count` calls.
///
/// Corresponds to Pekko's `OptimalSizeExploringResizer.ResizeRecord`. Pekko
/// uses `checkTime = 0L` as a sentinel meaning "no baseline has been recorded
/// yet". Because our [`Clock`] abstraction makes `Instant` an associated type,
/// an `Instant` value of "zero" is not necessarily invalid. Instead we carry
/// an explicit [`has_recorded`](Self::has_recorded) flag alongside a real
/// [`check_time`](Self::check_time) initialized at construction.
pub(crate) struct ResizeRecord<I> {
  /// Active under-utilization streak, if any.
  pub(crate) under_utilization_streak: Option<UnderUtilizationStreak<I>>,
  /// Cumulative message counter observed at the previous sample.
  pub(crate) message_count:            u64,
  /// Total pending mailbox size observed at the previous sample.
  pub(crate) total_queue_length:       u64,
  /// `true` once at least one sample has been recorded. Replaces Pekko's
  /// `checkTime > 0` gate that prevents perf_log updates from using an
  /// uninitialized baseline.
  pub(crate) has_recorded:             bool,
  /// Instant at which the previous sample was recorded.
  pub(crate) check_time:               I,
}

// --- Mutable state --------------------------------------------------------

/// Mutable bookkeeping protected by the resizer's spin mutex.
///
/// Exposed at `pub(crate)` visibility so that the parent resizer type, its
/// tests, and the crate-internal routing machinery can mutate fields via
/// `SpinSyncMutex::lock`.
pub(crate) struct State<I> {
  /// Historical mean processing time per pool size.
  pub(crate) performance_log: BTreeMap<usize, Duration>,
  /// Snapshot of the previous sample.
  pub(crate) record:          ResizeRecord<I>,
  /// Seedable pseudo-random source used for explore / optimize branching.
  pub(crate) rng:             Lcg,
}

/// Throughput-optimizing pool resizer.
///
/// Corresponds to Pekko's `DefaultOptimalSizeExploringResizer`. Periodically
/// chooses one of three resize actions based on per-routee mailbox
/// observations:
///
/// * **Downsize** — when the pool has not been fully utilized for longer than
///   [`downsize_after_underutilized_for`](Self::with_downsize_after_underutilized_for).
/// * **Explore** — with probability
///   [`exploration_probability`](Self::with_exploration_probability): step the pool size in a
///   random direction to discover new performance data.
/// * **Optimize** — otherwise: move the pool size halfway toward the best observed size within the
///   adjacency window.
///
/// The algorithm is stateful and tracks a running `performance_log` of
/// `current_size → mean_processing_duration` samples. Memory usage is
/// `O(upper_bound - lower_bound)`.
///
/// # Thread safety
///
/// All mutation happens inside a [`SpinSyncMutex`], so a single resizer
/// instance may be shared across concurrent dispatches via the router.
pub struct OptimalSizeExploringResizer<C: Clock> {
  lower_bound: usize,
  upper_bound: usize,
  chance_of_scaling_down_when_full: f64,
  action_interval: Duration,
  optimization_range: usize,
  explore_step_size: f64,
  downsize_ratio: f64,
  downsize_after_underutilized_for: Duration,
  exploration_probability: f64,
  weight_of_latest_metric: f64,
  clock: C,
  pub(crate) state: SpinSyncMutex<State<C::Instant>>,
}

impl<C: Clock> OptimalSizeExploringResizer<C> {
  /// Creates a new resizer with Pekko's default parameters and the given
  /// clock / RNG seed.
  ///
  /// # Panics
  ///
  /// * `lower_bound == 0`
  /// * `upper_bound < lower_bound`
  #[must_use]
  pub fn new(lower_bound: usize, upper_bound: usize, clock: C, seed: u64) -> Self {
    assert!(lower_bound > 0, "lower_bound must be positive");
    assert!(upper_bound >= lower_bound, "upper_bound must be >= lower_bound");
    let now = clock.now();
    let state = State {
      performance_log: BTreeMap::new(),
      record:          ResizeRecord {
        under_utilization_streak: None,
        message_count:            0,
        total_queue_length:       0,
        has_recorded:             false,
        check_time:               now,
      },
      rng:             Lcg::new(seed),
    };
    Self {
      lower_bound,
      upper_bound,
      // Pekko defaults (pekko.actor.deployment.default.optimal-size-exploring-resizer).
      chance_of_scaling_down_when_full: 0.2,
      action_interval: Duration::from_secs(5),
      optimization_range: 16,
      explore_step_size: 0.1,
      downsize_ratio: 0.8,
      downsize_after_underutilized_for: Duration::from_secs(72 * 60 * 60),
      exploration_probability: 0.4,
      weight_of_latest_metric: 0.5,
      clock,
      state: SpinSyncMutex::new(state),
    }
  }

  /// Overrides the chance of picking a downward step when exploring while
  /// fully utilized. Must be in `[0.0, 1.0]`.
  ///
  /// # Panics
  ///
  /// Panics if `value` is outside `[0.0, 1.0]`.
  #[must_use]
  pub fn with_chance_of_scaling_down_when_full(mut self, value: f64) -> Self {
    assert!((0.0..=1.0).contains(&value), "chance_of_scaling_down_when_full must be in [0.0, 1.0]");
    self.chance_of_scaling_down_when_full = value;
    self
  }

  /// Overrides the minimum interval between resize decisions.
  #[must_use]
  pub const fn with_action_interval(mut self, value: Duration) -> Self {
    self.action_interval = value;
    self
  }

  /// Overrides the number of pool sizes considered during optimization.
  ///
  /// Must be `>= 2`.
  ///
  /// # Panics
  ///
  /// Panics if `value < 2`.
  #[must_use]
  pub fn with_optimization_range(mut self, value: usize) -> Self {
    assert!(value >= 2, "optimization_range must be >= 2");
    self.optimization_range = value;
    self
  }

  /// Overrides the fractional step size used when exploring a new pool size.
  ///
  /// Must be `> 0.0` (matches Pekko's `checkParamAsPositiveNum`). With this
  /// bound, `ceil(current_size * explore_step_size) >= 1`, so the caller of
  /// [`Lcg::next_u32_bounded`](self::lcg::Lcg::next_u32_bounded) never reaches
  /// the zero-bound panic branch.
  ///
  /// # Panics
  ///
  /// Panics if `value <= 0.0`.
  #[must_use]
  pub fn with_explore_step_size(mut self, value: f64) -> Self {
    assert!(value > 0.0, "explore_step_size must be > 0.0");
    self.explore_step_size = value;
    self
  }

  /// Overrides the ratio applied to the highest observed utilization when
  /// downsizing. Must be in `[0.0, 1.0]`.
  ///
  /// # Panics
  ///
  /// Panics if `value` is outside `[0.0, 1.0]`.
  #[must_use]
  pub fn with_downsize_ratio(mut self, value: f64) -> Self {
    assert!((0.0..=1.0).contains(&value), "downsize_ratio must be in [0.0, 1.0]");
    self.downsize_ratio = value;
    self
  }

  /// Overrides the duration of under-utilization after which the pool is
  /// downsized.
  #[must_use]
  pub const fn with_downsize_after_underutilized_for(mut self, value: Duration) -> Self {
    self.downsize_after_underutilized_for = value;
    self
  }

  /// Overrides the probability of choosing `explore` over `optimize` when
  /// both are applicable. Must be in `[0.0, 1.0]`.
  ///
  /// # Panics
  ///
  /// Panics if `value` is outside `[0.0, 1.0]`.
  #[must_use]
  pub fn with_exploration_probability(mut self, value: f64) -> Self {
    assert!((0.0..=1.0).contains(&value), "exploration_probability must be in [0.0, 1.0]");
    self.exploration_probability = value;
    self
  }

  /// Overrides the exponential-moving-average weight applied to the latest
  /// performance sample. Must be in `[0.0, 1.0]`.
  ///
  /// # Panics
  ///
  /// Panics if `value` is outside `[0.0, 1.0]`.
  #[must_use]
  pub fn with_weight_of_latest_metric(mut self, value: f64) -> Self {
    assert!((0.0..=1.0).contains(&value), "weight_of_latest_metric must be in [0.0, 1.0]");
    self.weight_of_latest_metric = value;
    self
  }
}

impl<C: Clock> Resizer for OptimalSizeExploringResizer<C> {
  fn is_time_for_resize(&self, _message_counter: u64) -> bool {
    let check_time = self.state.lock().record.check_time;
    self.clock.elapsed_since(check_time) > self.action_interval
  }

  fn report_message_count(&self, mailbox_sizes: &[usize], message_counter: u64) {
    let current_size = mailbox_sizes.len();
    let total_queue_length: u64 = mailbox_sizes.iter().map(|n| *n as u64).sum();
    let utilized = mailbox_sizes.iter().filter(|n| **n > 0).count();
    let fully_utilized = !mailbox_sizes.is_empty() && utilized == current_size;
    let now = self.clock.now();

    let mut guard = self.state.lock();

    // Update the under-utilization streak.
    let new_streak = if fully_utilized {
      None
    } else {
      let (start, prior_highest) = match guard.record.under_utilization_streak {
        | Some(existing) => (existing.start, existing.highest_utilization),
        | None => (now, 0),
      };
      Some(UnderUtilizationStreak { start, highest_utilization: prior_highest.max(utilized) })
    };

    // Update the performance log when a full-utilization baseline exists.
    if fully_utilized && guard.record.under_utilization_streak.is_none() && guard.record.has_recorded {
      // Pekko 原典の `Int` 符号付減算（`OptimalSizeExploringResizer.scala:244-247`）を
      // 忠実に再現する。`queue_size_change` はキュー流入がメッセージ処理より速かった
      // 瞬間に負値となり、`total_processed <= 0` のとき perf_log の更新をスキップする。
      let total_message_received: i64 = message_counter.saturating_sub(guard.record.message_count) as i64;
      let queue_size_change: i64 = (guard.record.total_queue_length as i64) - (total_queue_length as i64);
      let total_processed: i64 = queue_size_change + total_message_received;
      if total_processed > 0 {
        let duration = self.clock.elapsed_since(guard.record.check_time);
        let last_nanos = duration.as_nanos() / (total_processed as u128);
        let last = Duration::from_nanos(last_nanos.min(u64::MAX as u128) as u64);
        let w = self.weight_of_latest_metric;
        let blended = match guard.performance_log.get(&current_size).copied() {
          | Some(old) => {
            let old_nanos = old.as_nanos() as f64;
            let new_nanos = last.as_nanos() as f64;
            let blended_nanos = old_nanos * (1.0 - w) + new_nanos * w;
            Duration::from_nanos(blended_nanos as u64)
          },
          | None => last,
        };
        guard.performance_log.insert(current_size, blended);
      }
    }

    guard.record = ResizeRecord {
      under_utilization_streak: new_streak,
      message_count: message_counter,
      total_queue_length,
      has_recorded: true,
      check_time: now,
    };
  }

  fn resize(&self, mailbox_sizes: &[usize]) -> i32 {
    let current_size = mailbox_sizes.len();
    let proposed_change = {
      let mut guard = self.state.lock();

      // Downsize when the streak has exceeded the configured threshold.
      let expired_streak = guard
        .record
        .under_utilization_streak
        .filter(|streak| self.clock.elapsed_since(streak.start) > self.downsize_after_underutilized_for);

      if let Some(streak) = expired_streak {
        let downsize_to = (streak.highest_utilization as f64 * self.downsize_ratio) as i64;
        let current = current_size as i64;
        (downsize_to - current).min(0) as i32
      } else if guard.performance_log.is_empty() || guard.record.under_utilization_streak.is_some() {
        0
      } else {
        // Branch between explore / optimize using the deterministic RNG.
        let roll = guard.rng.next_f64();
        if roll < self.exploration_probability {
          explore(&mut guard.rng, current_size, self.explore_step_size, self.chance_of_scaling_down_when_full)
        } else {
          optimize(&guard.performance_log, current_size, self.optimization_range)
        }
      }
    };

    let lower = self.lower_bound as i64;
    let upper = self.upper_bound as i64;
    let current = current_size as i64;
    let clamped = lower.max((proposed_change as i64 + current).min(upper));
    (clamped - current) as i32
  }
}

/// Picks a random step within `[1, ceil(current_size * explore_step_size)]`
/// and flips its sign according to `chance_of_scaling_down_when_full`.
fn explore(rng: &mut Lcg, current_size: usize, explore_step_size: f64, chance_of_scaling_down_when_full: f64) -> i32 {
  // `with_explore_step_size` で `explore_step_size > 0.0` を保証しているため、
  // `current_size >= 1` と合わせて `libm_ceil(...)` は必ず `>= 1` を返す。
  let bound = libm_ceil(current_size as f64 * explore_step_size) as u32;
  let raw = rng.next_u32_bounded(bound);
  let change = raw.max(1) as i32;
  if rng.next_f64() < chance_of_scaling_down_when_full { -change } else { change }
}

/// Moves halfway toward the pool size with the fastest observed mean
/// duration within the adjacency window.
fn optimize(perf_log: &BTreeMap<usize, Duration>, current_size: usize, optimization_range: usize) -> i32 {
  let num_each_side = optimization_range / 2;

  // Left boundary: up to `num_each_side` sizes strictly below current, ranked
  // by adjacency (smallest |current - size| first), then take the farthest.
  let mut lower_sizes: Vec<usize> = perf_log.keys().copied().filter(|s| *s < current_size).collect();
  lower_sizes.sort_by_key(|s| current_size - *s);
  let left_boundary = lower_sizes.into_iter().take(num_each_side).next_back().unwrap_or(current_size);

  let mut upper_sizes: Vec<usize> = perf_log.keys().copied().filter(|s| *s >= current_size).collect();
  upper_sizes.sort_by_key(|s| *s - current_size);
  let right_boundary = upper_sizes.into_iter().take(num_each_side).next_back().unwrap_or(current_size);

  let optimal_size = perf_log
    .iter()
    .filter(|(size, _)| **size >= left_boundary && **size <= right_boundary)
    .min_by_key(|(_, duration)| **duration)
    .map(|(size, _)| *size)
    .unwrap_or(current_size);

  let movement = (optimal_size as f64 - current_size as f64) / 2.0;
  if movement < 0.0 { libm_floor(movement) as i32 } else { libm_ceil(movement) as i32 }
}

// `core::f64::ceil` / `floor` は `std` 依存のため、`no_std` でビルドできるよう
// 符号付き整数キャストを用いて等価な挙動を手実装する。
fn libm_ceil(x: f64) -> f64 {
  let truncated = x as i64 as f64;
  if x > truncated { truncated + 1.0 } else { truncated }
}

fn libm_floor(x: f64) -> f64 {
  let truncated = x as i64 as f64;
  if x < truncated { truncated - 1.0 } else { truncated }
}
