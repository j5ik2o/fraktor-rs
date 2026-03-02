//! Pipe task abstraction storing pending futures for `pipe_to_self`.

use alloc::boxed::Box;
use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use crate::core::{
  actor::{ContextPipeTaskId, Pid, context_pipe_waker::ContextPipeWaker},
  messaging::AnyMessage,
  system::state::SystemStateShared,
};

/// Future type stored by context pipe tasks.
pub(crate) type ContextPipeFuture = Pin<Box<dyn Future<Output = AnyMessage> + Send + 'static>>;

/// Represents a pending `pipe_to_self` computation tracked by an actor cell.
pub(crate) struct ContextPipeTask {
  id:     ContextPipeTaskId,
  future: ContextPipeFuture,
  pid:    Pid,
  system: SystemStateShared,
}

impl ContextPipeTask {
  /// Creates a new context pipe task with the provided future.
  #[must_use]
  pub(crate) fn new(id: ContextPipeTaskId, future: ContextPipeFuture, pid: Pid, system: SystemStateShared) -> Self {
    Self { id, future, pid, system }
  }

  /// Returns the identifier of the task.
  #[must_use]
  pub(crate) const fn id(&self) -> ContextPipeTaskId {
    self.id
  }

  /// Polls the underlying future using a context pipe waker.
  pub(crate) fn poll(&mut self) -> Poll<AnyMessage> {
    let waker = ContextPipeWaker::into_waker(self.system.clone(), self.pid, self.id);
    let mut context = Context::from_waker(&waker);
    self.future.as_mut().poll(&mut context)
  }
}
