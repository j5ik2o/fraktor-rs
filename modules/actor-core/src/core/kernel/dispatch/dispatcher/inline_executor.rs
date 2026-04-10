//! Test-only [`Executor`] that runs tasks on the calling thread.

#[cfg(test)]
mod tests;

use alloc::{boxed::Box, collections::VecDeque};
use core::cell::RefCell;

use super::{execute_error::ExecuteError, executor::Executor};

/// Synchronous executor used by deterministic tests.
///
/// `InlineExecutor` runs each submitted task on the calling thread. To avoid
/// unbounded stack growth from re-entrant `execute` calls, it uses an internal
/// trampoline: the outermost call drains the queue while inner calls just
/// enqueue. The trampoline state is owned by the executor itself so it never
/// leaks into the [`super::ExecutorShared`] wrapper.
///
/// **Restriction**: this executor must not be installed inside an
/// [`super::ExecutorShared`] used by a production dispatcher. The shared
/// wrapper acquires `SpinSyncMutex` (which is non-reentrant) before delegating
/// to `Executor::execute`; running a task synchronously inside that lock would
/// deadlock as soon as the task tried to submit a follow-up task. Pekko's
/// `CallingThreadExecutor` is restricted in the same way.
pub struct InlineExecutor {
  state: RefCell<InlineState>,
}

struct InlineState {
  pending: VecDeque<Box<dyn FnOnce() + Send + 'static>>,
  running: bool,
}

impl Default for InlineExecutor {
  fn default() -> Self {
    Self::new()
  }
}

impl InlineExecutor {
  /// Creates a new inline executor with an empty trampoline queue.
  #[must_use]
  pub fn new() -> Self {
    Self { state: RefCell::new(InlineState { pending: VecDeque::new(), running: false }) }
  }
}

// SAFETY: `InlineExecutor` is not actually shareable across threads. The
// `Send + Sync` bound is required by the `Executor` trait, but the type is
// only sound when used from a single thread (test-only restriction). The
// `RefCell` ensures any cross-thread misuse is caught at borrow time rather
// than going silently undefined.
unsafe impl Send for InlineExecutor {}
unsafe impl Sync for InlineExecutor {}

impl Executor for InlineExecutor {
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>) -> Result<(), ExecuteError> {
    {
      let mut state = self.state.borrow_mut();
      state.pending.push_back(task);
      if state.running {
        return Ok(());
      }
      state.running = true;
    }

    loop {
      let next = self.state.borrow_mut().pending.pop_front();
      match next {
        | Some(task) => task(),
        | None => break,
      }
    }

    self.state.borrow_mut().running = false;
    Ok(())
  }

  fn shutdown(&mut self) {
    self.state.borrow_mut().pending.clear();
  }
}
