//! Handle describing which context-pipe task should be woken.

use crate::core::kernel::{
  actor::{Pid, context_pipe::ContextPipeTaskId},
  system::state::SystemStateShared,
};

/// Runtime handle identifying the task that a context-pipe waker should resume.
pub(crate) struct ContextPipeWakerHandle {
  pub(crate) system: SystemStateShared,
  pub(crate) pid:    Pid,
  pub(crate) task:   ContextPipeTaskId,
}

impl ContextPipeWakerHandle {
  /// Creates a new handle for the provided actor system, actor pid, and task id.
  #[must_use]
  pub(crate) const fn new(system: SystemStateShared, pid: Pid, task: ContextPipeTaskId) -> Self {
    Self { system, pid, task }
  }
}
