extern crate std;
use std::{string::String, thread};

use fraktor_actor_rs::core::kernel::dispatch::dispatcher::{DispatchError, DispatchExecutor, DispatchShared};

/// Executor that runs dispatcher batches on newly spawned OS threads.
pub struct ThreadedExecutor {
  name: Option<String>,
}

impl ThreadedExecutor {
  /// Creates an executor that spawns anonymous threads.
  #[must_use]
  pub const fn new() -> Self {
    Self { name: None }
  }
}

impl Default for ThreadedExecutor {
  fn default() -> Self {
    Self::new()
  }
}

impl DispatchExecutor for ThreadedExecutor {
  fn execute(&mut self, dispatcher: DispatchShared) -> Result<(), DispatchError> {
    let mut builder = thread::Builder::new();
    if let Some(name) = &self.name {
      builder = builder.name(name.clone());
    }

    builder.spawn(move || dispatcher.drive()).map(|_| ()).map_err(|_| DispatchError::RejectedExecution)
  }
}
