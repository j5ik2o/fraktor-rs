//! Runtime-owned mutable state for a live [`ActorCell`].

use alloc::{collections::VecDeque, string::String, vec::Vec};

use crate::core::{
  kernel::actor::{
    Pid, context_pipe::ContextPipeTask, messaging::AnyMessage, scheduler::SchedulerHandle,
    supervision::RestartStatistics,
  },
  typed::message_adapter::AdapterRefHandle,
};

/// Runtime-owned mutable state for a live [`ActorCell`].
pub(crate) struct ActorCellState {
  pub(crate) children:               Vec<Pid>,
  pub(crate) child_stats:            Vec<(Pid, RestartStatistics)>,
  pub(crate) watchers:               Vec<Pid>,
  pub(crate) watch_with_messages:    Vec<(Pid, AnyMessage)>,
  pub(crate) stashed_messages:       VecDeque<AnyMessage>,
  pub(crate) timer_handles:          Vec<(String, SchedulerHandle)>,
  pub(crate) pipe_tasks:             Vec<ContextPipeTask>,
  pub(crate) adapter_handles:        Vec<AdapterRefHandle>,
  pub(crate) adapter_handle_counter: u64,
  pub(crate) pipe_task_counter:      u64,
}

impl ActorCellState {
  /// Creates an empty runtime state for a freshly spawned actor cell.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self {
      children:               Vec::new(),
      child_stats:            Vec::new(),
      watchers:               Vec::new(),
      watch_with_messages:    Vec::new(),
      stashed_messages:       VecDeque::new(),
      timer_handles:          Vec::new(),
      pipe_tasks:             Vec::new(),
      adapter_handles:        Vec::new(),
      adapter_handle_counter: 0,
      pipe_task_counter:      0,
    }
  }
}

impl Default for ActorCellState {
  fn default() -> Self {
    Self::new()
  }
}
