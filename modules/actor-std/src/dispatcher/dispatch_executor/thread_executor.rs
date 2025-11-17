use std::{string::String, thread};

use fraktor_actor_core_rs::core::dispatcher::DispatchError;

use crate::dispatcher::{DispatchExecutor, DispatchShared};

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

  /// Assigns a thread name to future spawns.
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

impl DispatchExecutor for ThreadedExecutor {
  fn execute(&self, dispatcher: DispatchShared) -> Result<(), DispatchError> {
    let mut builder = thread::Builder::new();
    if let Some(name) = &self.name {
      builder = builder.name(name.clone());
    }

    builder.spawn(move || dispatcher.drive()).map(|_| ()).map_err(|_| DispatchError::RejectedExecution)
  }
}
