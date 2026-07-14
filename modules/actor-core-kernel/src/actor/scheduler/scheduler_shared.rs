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
  inner:           SharedRwLock<Scheduler>,
  observable_tick: ArcShared<AtomicU64>,
  resolution:      Duration,
}

impl Clone for SchedulerShared {
  fn clone(&self) -> Self {
    Self {
      inner:           self.inner.clone(),
      observable_tick: self.observable_tick.clone(),
      resolution:      self.resolution,
    }
  }
}

impl SchedulerShared {
  /// Wrap an existing shared rw lock.
  #[must_use]
  pub(crate) fn new(inner: SharedRwLock<Scheduler>) -> Self {
    let (observable_tick, resolution) =
      inner.with_read(|scheduler| (scheduler.observable_tick(), scheduler.resolution()));
    Self { inner, observable_tick, resolution }
  }

  /// Returns the scheduler's current logical time in whole seconds without
  /// building a diagnostics snapshot or acquiring the scheduler lock.
  #[must_use]
  pub fn current_time_secs(&self) -> u64 {
    let ticks = self.observable_tick.load(Ordering::Acquire);
    let nanos = self.resolution.as_nanos().saturating_mul(u128::from(ticks));
    u64::try_from(nanos / 1_000_000_000).unwrap_or(u64::MAX)
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
