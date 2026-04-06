extern crate std;

use std::{
  sync::mpsc::{Sender, channel},
  thread::{self, JoinHandle, ThreadId},
};

use fraktor_actor_core_rs::core::kernel::dispatch::dispatcher::{DispatchError, DispatchExecutor, DispatchShared};

/// Dedicated single-lane executor used by `PinnedDispatcher`.
pub struct PinnedExecutor {
  sender:    Option<Sender<DispatchShared>>,
  join:      Option<JoinHandle<()>>,
  thread_id: ThreadId,
}

impl PinnedExecutor {
  /// Spawns a dedicated worker thread.
  #[must_use]
  pub fn with_name(name: String) -> Self {
    let (tx, rx) = channel::<DispatchShared>();
    let builder = thread::Builder::new().name(name);
    let join = builder.spawn(move || {
      while let Ok(dispatcher) = rx.recv() {
        dispatcher.drive();
      }
    });
    let join = join.expect("pinned dispatcher worker thread must spawn");
    let thread_id = join.thread().id();
    Self { sender: Some(tx), join: Some(join), thread_id }
  }
}

impl DispatchExecutor for PinnedExecutor {
  fn execute(&mut self, dispatcher: DispatchShared) -> Result<(), DispatchError> {
    match &self.sender {
      | Some(sender) => sender.send(dispatcher).map_err(|_| DispatchError::ExecutorUnavailable),
      | None => Err(DispatchError::ExecutorUnavailable),
    }
  }
}

impl Drop for PinnedExecutor {
  fn drop(&mut self) {
    self.sender.take();
    if thread::current().id() == self.thread_id {
      let _ = self.join.take();
      return;
    }
    if let Some(join) = self.join.take() {
      // Best-effort worker teardown in Drop: a worker panic cannot be recovered here and does not affect
      // actor-system consistency after the executor has already been dropped.
      if let Err(_panic) = join.join() {}
    }
  }
}
