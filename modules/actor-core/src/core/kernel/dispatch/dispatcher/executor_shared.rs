//! Thin shared wrapper exposing [`Executor`] to multiple owners.
//!
//! `ExecutorShared` is the only sanctioned way to share a `Box<dyn Executor>`
//! between dispatchers and the rest of the runtime. Internal mutability is
//! confined to the `RuntimeMutex` housed inside `ArcShared`, matching the
//! AShared pattern documented in `docs/guides/shared_vs_handle.md`.
//!
//! # Re-entrant execution
//!
//! Production executors (Tokio / Threaded / Pinned) submit the task to a
//! worker and return quickly, so re-entrant `execute` calls from inside a
//! running task are harmless. Test-support executors that run the task
//! inline on the calling thread (for example `InlineExecutor`) would
//! normally deadlock on the inner `RuntimeMutex` when the running task calls
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

use alloc::{boxed::Box, collections::VecDeque};
use core::sync::atomic::{AtomicBool, Ordering};

use fraktor_utils_core_rs::core::sync::{ArcShared, RuntimeMutex, SharedAccess};

use super::{execute_error::ExecuteError, executor::Executor};
use crate::core::kernel::system::lock_provider::{DebugSpinLock, DebugSpinLockGuard};

type BoxedTask = Box<dyn FnOnce() + Send + 'static>;

struct TrampolineState {
  pending: VecDeque<BoxedTask>,
}

enum ExecutorGuard<'a> {
  Builtin(spin::MutexGuard<'a, Box<dyn Executor>>),
  Debug(DebugSpinLockGuard<'a, Box<dyn Executor>>),
}

impl core::ops::Deref for ExecutorGuard<'_> {
  type Target = Box<dyn Executor>;

  fn deref(&self) -> &Self::Target {
    match self {
      | Self::Builtin(guard) => guard,
      | Self::Debug(guard) => guard,
    }
  }
}

impl core::ops::DerefMut for ExecutorGuard<'_> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    match self {
      | Self::Builtin(guard) => guard,
      | Self::Debug(guard) => guard,
    }
  }
}

enum ExecutorLock {
  Builtin(ArcShared<RuntimeMutex<Box<dyn Executor>>>),
  Debug(ArcShared<DebugSpinLock<Box<dyn Executor>>>),
}

impl Clone for ExecutorLock {
  fn clone(&self) -> Self {
    match self {
      | Self::Builtin(inner) => Self::Builtin(inner.clone()),
      | Self::Debug(inner) => Self::Debug(inner.clone()),
    }
  }
}

impl ExecutorLock {
  fn builtin(executor: Box<dyn Executor>) -> Self {
    Self::Builtin(ArcShared::new(RuntimeMutex::new(executor)))
  }

  fn debug(executor: Box<dyn Executor>) -> Self {
    Self::Debug(ArcShared::new(DebugSpinLock::new(executor, "executor_shared.inner")))
  }

  fn lock(&self) -> ExecutorGuard<'_> {
    match self {
      | Self::Builtin(inner) => ExecutorGuard::Builtin(inner.lock()),
      | Self::Debug(inner) => ExecutorGuard::Debug(inner.lock()),
    }
  }
}

enum TrampolineLock {
  Builtin(ArcShared<RuntimeMutex<TrampolineState>>),
  Debug(ArcShared<DebugSpinLock<TrampolineState>>),
}

impl Clone for TrampolineLock {
  fn clone(&self) -> Self {
    match self {
      | Self::Builtin(inner) => Self::Builtin(inner.clone()),
      | Self::Debug(inner) => Self::Debug(inner.clone()),
    }
  }
}

impl TrampolineLock {
  fn builtin() -> Self {
    Self::Builtin(ArcShared::new(RuntimeMutex::new(TrampolineState { pending: VecDeque::new() })))
  }

  fn debug() -> Self {
    Self::Debug(ArcShared::new(DebugSpinLock::new(
      TrampolineState { pending: VecDeque::new() },
      "executor_shared.trampoline",
    )))
  }

  fn lock(&self) -> TrampolineGuard<'_> {
    match self {
      | Self::Builtin(inner) => TrampolineGuard::Builtin(inner.lock()),
      | Self::Debug(inner) => TrampolineGuard::Debug(inner.lock()),
    }
  }
}

enum TrampolineGuard<'a> {
  Builtin(spin::MutexGuard<'a, TrampolineState>),
  Debug(DebugSpinLockGuard<'a, TrampolineState>),
}

impl core::ops::Deref for TrampolineGuard<'_> {
  type Target = TrampolineState;

  fn deref(&self) -> &Self::Target {
    match self {
      | Self::Builtin(guard) => guard,
      | Self::Debug(guard) => guard,
    }
  }
}

impl core::ops::DerefMut for TrampolineGuard<'_> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    match self {
      | Self::Builtin(guard) => guard,
      | Self::Debug(guard) => guard,
    }
  }
}

/// Multi-owner handle for a boxed [`Executor`].
///
/// Cloning the wrapper is cheap (`ArcShared::clone`) and does not duplicate the
/// underlying executor. The inner executor mutex is held only for each
/// individual `inner.execute(task)` invocation, not across the full queue
/// drain — this keeps re-entrant inline executors deadlock-free.
pub struct ExecutorShared {
  inner:      ExecutorLock,
  trampoline: TrampolineLock,
  running:    ArcShared<AtomicBool>,
}

impl ExecutorShared {
  /// Wraps the provided executor in a shareable handle.
  #[must_use]
  pub fn new<E: Executor + 'static>(executor: E) -> Self {
    Self::from_boxed(Box::new(executor))
  }

  /// Wraps an already-boxed executor in a shareable handle.
  #[must_use]
  pub fn from_boxed(executor: Box<dyn Executor>) -> Self {
    Self {
      inner:      ExecutorLock::builtin(executor),
      trampoline: TrampolineLock::builtin(),
      running:    ArcShared::new(AtomicBool::new(false)),
    }
  }

  pub(crate) fn from_boxed_debug(executor: Box<dyn Executor>) -> Self {
    Self {
      inner:      ExecutorLock::debug(executor),
      trampoline: TrampolineLock::debug(),
      running:    ArcShared::new(AtomicBool::new(false)),
    }
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
  pub fn execute(&self, task: BoxedTask) -> Result<(), ExecuteError> {
    // Phase 1: queue the task.
    self.trampoline.lock().pending.push_back(task);

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
      let next = self.trampoline.lock().pending.pop_front();
      match next {
        | Some(task) => {
          let result = self.with_write(|inner| inner.execute(task));
          if let Err(err) = result {
            last_err = Some(err);
            // Drop remaining queued tasks: the executor is in a bad state.
            self.trampoline.lock().pending.clear();
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
      && !self.trampoline.lock().pending.is_empty()
      && self.running.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_ok()
    {
      // Tail drain: re-use the same loop body on the thin remaining tail.
      loop {
        let next = self.trampoline.lock().pending.pop_front();
        match next {
          | Some(task) => {
            let result = self.with_write(|inner| inner.execute(task));
            if let Err(err) = result {
              last_err = Some(err);
              self.trampoline.lock().pending.clear();
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

  /// Shuts the inner executor down.
  pub fn shutdown(&self) {
    self.with_write(|inner| inner.shutdown());
    self.trampoline.lock().pending.clear();
  }
}

impl Clone for ExecutorShared {
  fn clone(&self) -> Self {
    Self { inner: self.inner.clone(), trampoline: self.trampoline.clone(), running: self.running.clone() }
  }
}

impl SharedAccess<Box<dyn Executor>> for ExecutorShared {
  fn with_read<R>(&self, f: impl FnOnce(&Box<dyn Executor>) -> R) -> R {
    let guard = self.inner.lock();
    f(&guard)
  }

  fn with_write<R>(&self, f: impl FnOnce(&mut Box<dyn Executor>) -> R) -> R {
    let mut guard = self.inner.lock();
    f(&mut guard)
  }
}
