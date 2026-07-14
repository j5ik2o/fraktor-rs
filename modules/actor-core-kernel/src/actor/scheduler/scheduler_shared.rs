//! Thin shared wrapper for `Scheduler`.
//!
//! Hides the `SharedRwLock<...>` internals and exposes only
//! the `with_read` / `with_write` closure API.

#[cfg(test)]
#[path = "scheduler_shared_test.rs"]
mod tests;

use alloc::{boxed::Box, vec::Vec};
use core::{mem, time::Duration};

use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedAccess, SharedLock, SharedRwLock};
use portable_atomic::{AtomicBool, AtomicU64, Ordering};

use super::Scheduler;

type SchedulerAfterWriteAction = Box<dyn FnOnce() + Send + 'static>;

struct SchedulerWriteGuard<'a> {
  active:   &'a AtomicBool,
  actions:  &'a SharedLock<Vec<SchedulerAfterWriteAction>>,
  finished: bool,
}

impl<'a> SchedulerWriteGuard<'a> {
  fn new(active: &'a AtomicBool, actions: &'a SharedLock<Vec<SchedulerAfterWriteAction>>) -> Self {
    active.store(true, Ordering::Release);
    Self { active, actions, finished: false }
  }

  fn finish(mut self) -> Vec<SchedulerAfterWriteAction> {
    let actions = self.actions.with_write(|actions| {
      self.active.store(false, Ordering::Release);
      mem::take(actions)
    });
    self.finished = true;
    actions
  }
}

impl Drop for SchedulerWriteGuard<'_> {
  fn drop(&mut self) {
    if !self.finished {
      self.actions.with_write(|actions| {
        self.active.store(false, Ordering::Release);
        actions.clear();
      });
    }
  }
}

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
  inner:               SharedRwLock<Scheduler>,
  current_tick:        ArcShared<AtomicU64>,
  resolution:          Duration,
  write_in_progress:   ArcShared<AtomicBool>,
  after_write_actions: SharedLock<Vec<SchedulerAfterWriteAction>>,
}

impl Clone for SchedulerShared {
  fn clone(&self) -> Self {
    Self {
      inner:               self.inner.clone(),
      current_tick:        self.current_tick.clone(),
      resolution:          self.resolution,
      write_in_progress:   self.write_in_progress.clone(),
      after_write_actions: self.after_write_actions.clone(),
    }
  }
}

impl SchedulerShared {
  /// Wrap an existing shared rw lock.
  #[must_use]
  pub(crate) fn new(inner: SharedRwLock<Scheduler>) -> Self {
    let (current_tick, resolution) =
      inner.with_read(|scheduler| (scheduler.current_tick_snapshot(), scheduler.resolution()));
    Self {
      inner,
      current_tick,
      resolution,
      write_in_progress: ArcShared::new(AtomicBool::new(false)),
      after_write_actions: SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new()),
    }
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

  /// Runs an action after an in-progress scheduler write releases its lock.
  ///
  /// The action runs immediately when no scheduler write is active. Scheduler
  /// callbacks can use this method to move re-entrant external work outside
  /// the scheduler lock without delaying ordinary call sites.
  pub fn run_after_write(&self, action: impl FnOnce() + Send + 'static) {
    let mut action: Option<SchedulerAfterWriteAction> = Some(Box::new(action));
    self.after_write_actions.with_write(|actions| {
      if self.write_in_progress.load(Ordering::Acquire)
        && let Some(action) = action.take()
      {
        actions.push(action);
      }
    });
    if let Some(action) = action {
      action();
    }
  }
}

impl SharedAccess<Scheduler> for SchedulerShared {
  #[inline]
  fn with_read<R>(&self, f: impl FnOnce(&Scheduler) -> R) -> R {
    self.inner.with_read(f)
  }

  #[inline]
  fn with_write<R>(&self, f: impl FnOnce(&mut Scheduler) -> R) -> R {
    let (result, actions) = self.inner.with_write(|scheduler| {
      let guard = SchedulerWriteGuard::new(&self.write_in_progress, &self.after_write_actions);
      let result = f(scheduler);
      (result, guard.finish())
    });
    for action in actions {
      action();
    }
    result
  }
}
