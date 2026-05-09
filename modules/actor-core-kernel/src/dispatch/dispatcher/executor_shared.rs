//! Thin shared wrapper exposing [`Executor`] to multiple owners.
//!
//! `ExecutorShared` is the only sanctioned way to share a `Box<dyn Executor>`
//! between dispatchers and the rest of the runtime. Internal mutability is
//! confined to the `SpinSyncMutex` housed inside `ArcShared`, matching the
//! AShared pattern documented in `docs/guides/shared_vs_handle.md`.
//!
//! # Re-entrant execution
//!
//! Production executors (Tokio / Threaded / Pinned) submit the task to a
//! worker and return quickly, so re-entrant `execute` calls from inside a
//! running task are harmless. Test-support executors that run the task
//! inline on the calling thread (for example `InlineExecutor`) would
//! normally deadlock on the inner `SpinSyncMutex` when the running task calls
//! `execute` again. To keep both families working, `ExecutorShared` runs its
//! own outer trampoline: the caller that first sees the queue idle becomes
//! the drain owner and processes every queued task one-by-one while
//! subsequent re-entrant callers simply push to the queue and return.
//! Because the trampoline uses a separate mutex, the drain loop holds the
//! inner executor mutex only during each individual `inner.execute(task)`
//! call, so nested calls from inside an inline task don't touch the same
//! lock.

#[cfg(test)]
mod tests;

use alloc::boxed::Box;
use core::sync::atomic::{AtomicBool, Ordering};

use fraktor_utils_core_rs::sync::{ArcShared, DefaultMutex, SharedAccess, SharedLock};

use super::{
  drive_guard_token::DriveGuardToken,
  execute_error::ExecuteError,
  executor::Executor,
  trampoline_state::{QueuedTask, TrampolineState},
};

type BoxedTask = Box<dyn FnOnce() + Send + 'static>;

/// Multi-owner handle for a boxed [`Executor`].
///
/// Cloning the wrapper is cheap (`ArcShared::clone`) and does not duplicate the
/// underlying executor. The inner executor mutex is held only for each
/// individual `inner.execute(task)` invocation, not across the full queue
/// drain — this keeps re-entrant inline executors deadlock-free.
pub struct ExecutorShared {
  inner:      SharedLock<Box<dyn Executor>>,
  trampoline: SharedLock<TrampolineState>,
  running:    ArcShared<AtomicBool>,
}

impl ExecutorShared {
  /// Creates a new shared wrapper using the builtin spin lock backend.
  #[must_use]
  pub fn new(executor: Box<dyn Executor>, trampoline: TrampolineState) -> Self {
    Self::from_shared_lock(
      SharedLock::new_with_driver::<DefaultMutex<_>>(executor),
      SharedLock::new_with_driver::<DefaultMutex<_>>(trampoline),
    )
  }

  /// Wraps already constructed shared locks in a shareable handle.
  #[must_use]
  pub fn from_shared_lock(inner: SharedLock<Box<dyn Executor>>, trampoline: SharedLock<TrampolineState>) -> Self {
    Self { inner, trampoline, running: ArcShared::new(AtomicBool::new(false)) }
  }

  /// Submits the task to the inner executor.
  ///
  /// The task is queued on the trampoline first so that re-entrant calls
  /// from inside an inline executor just append to the queue and return
  /// without trying to re-acquire the inner executor mutex.
  ///
  /// # Errors
  ///
  /// Returns [`ExecuteError`] when the underlying executor rejects the task.
  pub fn execute(&self, task: BoxedTask, affinity_key: u64) -> Result<(), ExecuteError> {
    // Phase 1: queue the task.
    self.trampoline.with_lock(|state| state.pending.push_back(QueuedTask { task, affinity_key }));

    // Phase 2: become the drain owner. If someone else is already draining,
    // we simply return after queuing — they will pick up our task.
    if self.running.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() {
      return Ok(());
    }

    // Phase 3: drain the queue. We pop one task at a time and submit it to
    // the inner executor. The inner mutex is acquired/released for each
    // task, so re-entrant calls from an inline task don't deadlock.
    let mut last_err: Option<ExecuteError> = None;
    loop {
      let next = self.trampoline.with_lock(|state| state.pending.pop_front());
      match next {
        | Some(queued) => {
          let result = self.with_write(|inner| inner.execute(queued.task, queued.affinity_key));
          if let Err(err) = result {
            last_err = Some(err);
            // Drop remaining queued tasks: the executor is in a bad state.
            self.trampoline.with_lock(|state| state.pending.clear());
            break;
          }
        },
        | None => break,
      }
    }

    self.running.store(false, Ordering::Release);

    // Step 4: re-check the queue in case a concurrent producer arrived
    // between the last pop and our `running = false` release. If so,
    // another caller should still be able to drain (they'll CAS
    // successfully), but to avoid lost wake-ups we nudge once here.
    if last_err.is_none()
      && self.trampoline.with_read(|state| !state.pending.is_empty())
      && self.running.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok()
    {
      // Tail drain: re-use the same loop body on the thin remaining tail.
      loop {
        let next = self.trampoline.with_lock(|state| state.pending.pop_front());
        match next {
          | Some(queued) => {
            let result = self.with_write(|inner| inner.execute(queued.task, queued.affinity_key));
            if let Err(err) = result {
              last_err = Some(err);
              self.trampoline.with_lock(|state| state.pending.clear());
              break;
            }
          },
          | None => break,
        }
      }
      self.running.store(false, Ordering::Release);
    }

    match last_err {
      | Some(err) => Err(err),
      | None => Ok(()),
    }
  }

  /// Claims the drain-owner slot on this shared executor so that subsequent
  /// [`execute`](Self::execute) calls made while the returned
  /// [`DriveGuardToken`] is alive simply enqueue into the trampoline and
  /// return without draining.
  ///
  /// If another drain owner is already active, the CAS fails and the returned
  /// token reports `claimed = false`; its `Drop` is then a no-op so the outer
  /// owner keeps running the drain loop. This contention handling is the
  /// reason nested guards and production multi-thread access are safe without
  /// additional synchronisation.
  ///
  /// Pairs with the token's `Drop` implementation. There is intentionally no
  /// public `exit_drive_guard` method: release only happens through `Drop`, so
  /// the type system prevents release-forgotten / double-release bugs.
  ///
  /// # Use case
  ///
  /// `ActorCell::fault_recreate` wraps the `pre_restart` call with this guard
  /// (via [`MessageDispatcherShared::run_with_drive_guard`](super::MessageDispatcherShared::run_with_drive_guard))
  /// so that default `pre_restart`'s `stop_all_children` does not trigger
  /// same-thread reentrancy into child mailboxes when the invocation is
  /// driven from outside the normal `execute` path (e.g. test direct
  /// `system_invoke` calls).
  pub(crate) fn enter_drive_guard(&self) -> DriveGuardToken {
    let claimed = self.running.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok();
    DriveGuardToken::new(claimed, self.running.clone())
  }

  /// Shuts the inner executor down.
  pub fn shutdown(&self) {
    self.with_write(|inner| inner.shutdown());
    self.trampoline.with_lock(|state| state.pending.clear());
  }
}

impl Clone for ExecutorShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), trampoline: self.trampoline.clone(), running: self.running.clone() }
  }
}

impl SharedAccess<Box<dyn Executor>> for ExecutorShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn Executor>) -> R) -> R {
    self.inner.with_read(|guard| f(guard))
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn Executor>) -> R) -> R {
    self.inner.with_lock(|guard| f(guard))
  }
}
