//! Internal termination state for the actor system.

use alloc::vec::Vec;
use core::{
  sync::atomic::{AtomicBool, Ordering},
  task::Waker,
};

use fraktor_utils_core_rs::sync::{DefaultMutex, SharedAccess, SharedLock};

/// Tracks actor system termination as the single source of truth.
///
/// This type replaces the previous pattern where `SystemState` held separate
/// `AtomicBool` flags and an `ActorFutureShared<()>`. It provides non-consuming
/// observation: multiple observers can concurrently check or await termination
/// without interfering with each other.
pub(crate) struct TerminationState {
  terminating: AtomicBool,
  terminated:  AtomicBool,
  wakers:      SharedLock<Vec<Waker>>,
}

impl TerminationState {
  /// Creates a new state in the not-yet-terminating condition.
  #[must_use]
  pub(crate) fn new() -> Self {
    Self {
      terminating: AtomicBool::new(false),
      terminated:  AtomicBool::new(false),
      wakers:      SharedLock::new_with_driver::<DefaultMutex<_>>(Vec::new()),
    }
  }

  /// Attempts to transition into the terminating state.
  ///
  /// Returns `true` if this call initiated the transition, `false` if another
  /// caller has already done so.
  pub(crate) fn begin_termination(&self) -> bool {
    !self.terminating.swap(true, Ordering::AcqRel)
  }

  /// Indicates whether the system is currently terminating.
  #[must_use]
  pub(crate) fn is_terminating(&self) -> bool {
    self.terminating.load(Ordering::Acquire)
  }

  /// Indicates whether the system has fully terminated.
  #[must_use]
  pub(crate) fn is_terminated(&self) -> bool {
    self.terminated.load(Ordering::Acquire)
  }

  /// Marks the system as terminated and wakes all registered observers.
  ///
  /// This method is idempotent: only the first call performs the transition
  /// and wakes observers.
  pub(crate) fn mark_terminated(&self) {
    self.terminating.store(true, Ordering::Release);
    if self.terminated.swap(true, Ordering::AcqRel) {
      return;
    }
    // ロック中に waker を取り出し、ロック外で wake してデッドロックを避ける
    let wakers = self.wakers.with_write(core::mem::take);
    for w in wakers {
      w.wake();
    }
  }

  /// Registers a waker to be notified when termination completes.
  ///
  /// If the system is already terminated, the waker is woken immediately.
  pub(crate) fn register_waker(&self, waker: &Waker) {
    if self.is_terminated() {
      waker.wake_by_ref();
      return;
    }
    let should_wake = self.wakers.with_write(|guard| {
      // Double-check after acquiring the lock to avoid lost wakeups.
      if self.terminated.load(Ordering::Acquire) {
        return true;
      }
      if !guard.iter().any(|registered| registered.will_wake(waker)) {
        guard.push(waker.clone());
      }
      false
    });
    if should_wake {
      waker.wake_by_ref();
    }
  }
}
