//! [`Executor`] that spawns a fresh OS thread per submitted task.

#[cfg(test)]
mod tests;

extern crate std;

use alloc::boxed::Box;
use std::{string::String, thread::Builder};

use fraktor_actor_core_rs::core::kernel::dispatch::dispatcher::{ExecuteError, Executor};

/// Spawns each submitted task on a brand-new OS thread.
///
/// Useful for blocking workloads where one task may park its thread for an
/// arbitrary duration. The executor itself maintains no per-task state, so
/// `shutdown` is a best-effort no-op.
pub struct ThreadedExecutor {
  name: Option<String>,
}

impl ThreadedExecutor {
  /// Creates an executor that spawns anonymous threads.
  #[must_use]
  pub const fn new() -> Self {
    Self { name: None }
  }

  /// Creates an executor that names spawned threads with the supplied prefix.
  #[must_use]
  pub fn with_name(name: impl Into<String>) -> Self {
    Self { name: Some(name.into()) }
  }
}

impl Default for ThreadedExecutor {
  fn default() -> Self {
    Self::new()
  }
}

impl Executor for ThreadedExecutor {
  fn execute(&mut self, task: Box<dyn FnOnce() + Send + 'static>, _affinity_key: u64) -> Result<(), ExecuteError> {
    let mut builder = Builder::new();
    if let Some(name) = &self.name {
      builder = builder.name(name.clone());
    }
    builder.spawn(task).map(|_handle| ()).map_err(|err| ExecuteError::Backend(alloc::format!("{err}")))
  }

  fn shutdown(&mut self) {
    // Nothing to release; spawned threads are owned by the operating system
    // until they exit on their own.
  }
}
