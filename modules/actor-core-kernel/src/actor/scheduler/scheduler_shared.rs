//! Thin shared wrapper for `Scheduler`.
//!
//! Hides the `SharedRwLock<...>` internals and exposes only
//! the `with_read` / `with_write` closure API.

use core::time::Duration;

use fraktor_utils_core_rs::sync::{ArcShared, SharedAccess, SharedRwLock};
use portable_atomic::{AtomicU64, Ordering};

use super::Scheduler;

/// Thin shared wrapper around [`SharedRwLock<Scheduler>`].
///
/// External callers obtain this handle from
/// [`crate::actor::scheduler::SchedulerContext`] instead of
/// constructing it from the raw lock.
///
/// ```compile_fail
/// use fraktor_actor_core_kernel_rs::actor::scheduler::{Scheduler, SchedulerConfig, SchedulerShared};
/// use fraktor_utils_core_rs::sync::{DefaultRwLock, SharedRwLock};
///
/// let scheduler = Scheduler::new(SchedulerConfig::default());
/// let _ = SchedulerShared::new(SharedRwLock::new_with_driver::<DefaultRwLock<_>>(scheduler));
/// ```
pub struct SchedulerShared {
  inner:        SharedRwLock<Scheduler>,
  current_tick: ArcShared<AtomicU64>,
  resolution:   Duration,
}

impl Clone for SchedulerShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), current_tick: self.current_tick.clone(), resolution: self.resolution }
  }
}

impl SchedulerShared {
  /// Wrap an existing shared rw lock.
  #[must_use]
  pub(crate) fn new(inner: SharedRwLock<Scheduler>) -> Self {
    let (current_tick, resolution) =
      inner.with_read(|scheduler| (scheduler.current_tick_snapshot(), scheduler.resolution()));
    Self { inner, current_tick, resolution }
  }

  /// Returns the scheduler's current logical time rounded up to whole seconds,
  /// without building a diagnostics snapshot.
  #[must_use]
  pub fn current_time_secs(&self) -> u64 {
    let nanos = self.resolution.as_nanos().saturating_mul(u128::from(self.current_tick.load(Ordering::Acquire)));
    u64::try_from(nanos.div_ceil(1_000_000_000)).unwrap_or(u64::MAX)
  }

  /// Returns the longest delay accepted by this scheduler resolution.
  #[must_use]
  pub fn maximum_delay(&self) -> Duration {
    self.resolution.checked_mul(i32::MAX as u32).unwrap_or(Duration::MAX)
  }
}

impl SharedAccess<Scheduler> for SchedulerShared {
  #[inline]
  fn with_read<R>(&self, f: impl FnOnce(&Scheduler) -> R) -> R {
    self.inner.with_read(f)
  }

  #[inline]
  fn with_write<R>(&self, f: impl FnOnce(&mut Scheduler) -> R) -> R {
    self.inner.with_write(f)
  }
}
