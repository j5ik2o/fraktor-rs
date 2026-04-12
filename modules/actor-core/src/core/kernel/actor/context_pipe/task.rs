//! Pipe task abstraction storing pending futures for `pipe_to_self`.

use alloc::boxed::Box;
use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use crate::core::kernel::{
  actor::{
    Pid,
    actor_ref::ActorRef,
    context_pipe::{ContextPipeTaskId, ContextPipeWaker, ContextPipeWakerHandle},
    messaging::AnyMessage,
  },
  system::state::SystemStateShared,
};

/// Future type stored by context pipe tasks.
///
/// `None` means the asynchronous result was intentionally dropped after it had
/// already been observed (for example after logging an adapter failure).
pub(crate) type ContextPipeFuture = Pin<Box<dyn Future<Output = Option<AnyMessage>> + Send + 'static>>;

/// Represents a pending `pipe_to_self` or `pipe_to` computation tracked by an actor cell.
pub(crate) struct ContextPipeTask {
  id:              ContextPipeTaskId,
  future:          ContextPipeFuture,
  pid:             Pid,
  system:          SystemStateShared,
  delivery_target: Option<ActorRef>,
}

impl ContextPipeTask {
  /// Creates a new context pipe task targeting the actor itself (`pipe_to_self`).
  #[must_use]
  pub(crate) fn new(id: ContextPipeTaskId, future: ContextPipeFuture, pid: Pid, system: SystemStateShared) -> Self {
    Self { id, future, pid, system, delivery_target: None }
  }

  /// Creates a new context pipe task targeting an external actor (`pipe_to`).
  #[must_use]
  pub(crate) fn new_with_target(
    id: ContextPipeTaskId,
    future: ContextPipeFuture,
    pid: Pid,
    system: SystemStateShared,
    target: ActorRef,
  ) -> Self {
    Self { id, future, pid, system, delivery_target: Some(target) }
  }

  /// Returns the identifier of the task.
  #[must_use]
  pub(crate) const fn id(&self) -> ContextPipeTaskId {
    self.id
  }

  /// Takes the delivery target, leaving `None` in its place.
  pub(crate) const fn take_delivery_target(&mut self) -> Option<ActorRef> {
    self.delivery_target.take()
  }

  /// Polls the underlying future using a context pipe waker.
  pub(crate) fn poll(&mut self) -> Poll<Option<AnyMessage>> {
    let handle = ContextPipeWakerHandle::new(self.system.clone(), self.pid, self.id);
    let shared = self.system.context_pipe_waker_handle_shared_factory().create_context_pipe_waker_handle_shared(handle);
    let waker = ContextPipeWaker::into_waker(shared);
    let mut context = Context::from_waker(&waker);
    self.future.as_mut().poll(&mut context)
  }
}
