//! Pipe task abstraction storing pending futures for `pipe_to_self`.

use alloc::boxed::Box;
use core::{
  future::Future,
  pin::Pin,
  task::{Context, Poll},
};

use fraktor_utils_core_rs::core::{runtime_toolbox::RuntimeToolbox, sync::ArcShared};

use crate::core::{
  actor_prim::{ContextPipeTaskId, Pid, context_pipe_waker::ContextPipeWaker},
  messaging::AnyMessageGeneric,
  system::SystemStateGeneric,
};

/// Future type stored by context pipe tasks.
pub(crate) type ContextPipeFuture<TB> = Pin<Box<dyn Future<Output = AnyMessageGeneric<TB>> + Send + 'static>>;

/// Represents a pending `pipe_to_self` computation tracked by an actor cell.
pub(crate) struct ContextPipeTask<TB: RuntimeToolbox + 'static> {
  id:     ContextPipeTaskId,
  future: ContextPipeFuture<TB>,
  pid:    Pid,
  system: ArcShared<SystemStateGeneric<TB>>,
}

impl<TB: RuntimeToolbox + 'static> ContextPipeTask<TB> {
  /// Creates a new context pipe task with the provided future.
  #[must_use]
  pub(crate) fn new(
    id: ContextPipeTaskId,
    future: ContextPipeFuture<TB>,
    pid: Pid,
    system: ArcShared<SystemStateGeneric<TB>>,
  ) -> Self {
    Self { id, future, pid, system }
  }

  /// Returns the identifier of the task.
  #[must_use]
  pub(crate) const fn id(&self) -> ContextPipeTaskId {
    self.id
  }

  /// Polls the underlying future using a context pipe waker.
  pub(crate) fn poll(&mut self) -> Poll<AnyMessageGeneric<TB>> {
    let waker = ContextPipeWaker::<TB>::into_waker(self.system.clone(), self.pid, self.id);
    let mut context = Context::from_waker(&waker);
    self.future.as_mut().poll(&mut context)
  }
}
