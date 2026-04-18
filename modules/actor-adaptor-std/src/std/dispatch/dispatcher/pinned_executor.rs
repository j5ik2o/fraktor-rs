//! Single-thread dedicated [`Executor`] used by `PinnedDispatcher`.

#[cfg(test)]
mod tests;

extern crate std;

use alloc::boxed::Box;
use std::{
  string::String,
  sync::mpsc::{Sender, channel},
  thread::{self, Builder, JoinHandle, ThreadId},
};

use fraktor_actor_core_rs::core::kernel::dispatch::dispatcher::{ExecuteError, Executor};

type Task = Box<dyn FnOnce() + Send + 'static>;

/// Spawns a single dedicated worker thread that runs every submitted task.
///
/// The Pekko equivalent is `org.apache.pekko.dispatch.PinnedDispatcher`'s
/// underlying executor: at most one task is in flight at a time, and shutdown
/// joins the worker thread cleanly.
pub struct PinnedExecutor {
  sender:    Option<Sender<Task>>,
  join:      Option<JoinHandle<()>>,
  thread_id: Option<ThreadId>,
}

impl PinnedExecutor {
  /// Spawns the worker thread with the supplied name.
  ///
  /// # Panics
  ///
  /// Panics if the worker thread cannot be spawned.
  #[must_use]
  pub fn with_name(name: impl Into<String>) -> Self {
    let (tx, rx) = channel::<Task>();
    let builder = Builder::new().name(name.into());
    let join = builder
      .spawn(move || {
        while let Ok(task) = rx.recv() {
          task();
        }
      })
      .expect("pinned executor worker thread must spawn");
    let thread_id = Some(join.thread().id());
    Self { sender: Some(tx), join: Some(join), thread_id }
  }
}

impl Executor for PinnedExecutor {
  fn execute(&mut self, task: Task, _affinity_key: u64) -> Result<(), ExecuteError> {
    let Some(sender) = self.sender.as_ref() else {
      return Err(ExecuteError::Shutdown);
    };
    sender.send(task).map_err(|_| ExecuteError::Shutdown)
  }

  fn shutdown(&mut self) {
    self.sender.take();
    let same_thread = self.thread_id.is_some_and(|id| id == thread::current().id());
    if same_thread {
      // Cannot join from inside the worker thread; release the handle.
      drop(self.join.take());
      return;
    }
    if let Some(join) = self.join.take() {
      // Best-effort shutdown: a worker panic cannot be recovered here, so the
      // join result is intentionally observed-and-ignored to satisfy the
      // `#[must_use]` contract.
      drop(join.join());
    }
  }
}

impl Drop for PinnedExecutor {
  fn drop(&mut self) {
    self.shutdown();
  }
}
