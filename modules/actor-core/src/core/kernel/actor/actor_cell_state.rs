//! Runtime-owned mutable state for a live [`ActorCell`].

use alloc::{collections::VecDeque, string::String, vec::Vec};

use crate::core::{
  kernel::actor::{
    ChildrenContainer, FailedInfo, Pid, context_pipe::ContextPipeTask, error::ActorErrorReason, messaging::AnyMessage,
    scheduler::SchedulerHandle,
  },
  typed::message_adapter::AdapterRefHandle,
};

/// Runtime-owned mutable state for a live [`ActorCell`].
pub(crate) struct ActorCellState {
  /// Child registry state machine (Pekko parity: `ChildrenContainer`).
  ///
  /// Supersedes the previous `children: Vec<Pid>` + `child_stats:
  /// Vec<(Pid, RestartStatistics)>` pair — both are now stored inside the
  /// [`ChildrenContainer`] and kept in lockstep by the container's API.
  pub(crate) children_state:         ChildrenContainer,
  /// Failure state tag corresponding to Pekko's private `_failed` field.
  ///
  /// AC-H3 extension (Pekko `FaultHandling.scala`): tracks whether the cell is
  /// currently processing a failure and, if so, whether the failure was caused
  /// by a child (`FailedRef(perpetrator)`) or was fatal (`FailedFatally`).
  pub(crate) failed:                 FailedInfo,
  pub(crate) watchers:               Vec<Pid>,
  /// Pids this cell is watching (Pekko `DeathWatch.scala` `watching`).
  ///
  /// AC-H5: populated by `register_watching` and drained by `unregister_watching`.
  pub(crate) watching:               Vec<Pid>,
  /// Pids whose `Terminated` has been enqueued on the user mailbox but has not
  /// yet been dispatched to the user's `on_terminated` handler (Pekko
  /// `DeathWatch.scala` `terminatedQueued`).
  ///
  /// AC-H5: dedup key for `DeathWatchNotification` to prevent double user-queue
  /// delivery.
  pub(crate) terminated_queued:      Vec<Pid>,
  pub(crate) watch_with_messages:    Vec<(Pid, AnyMessage)>,
  pub(crate) stashed_messages:       VecDeque<AnyMessage>,
  pub(crate) timer_handles:          Vec<(String, SchedulerHandle)>,
  pub(crate) pipe_tasks:             Vec<ContextPipeTask>,
  pub(crate) adapter_handles:        Vec<AdapterRefHandle>,
  pub(crate) adapter_handle_counter: u64,
  pub(crate) pipe_task_counter:      u64,
  /// Cached `Recreation` cause while the cell waits for its live children to
  /// die before running `finish_recreate` (Pekko `FaultHandling.scala:215-237`).
  ///
  /// AC-H4: stored once by `fault_recreate` and consumed by `finish_recreate`
  /// (either immediately when no children live, or lazily when the last child
  /// terminates). `None` outside of a restart sequence.
  pub(crate) deferred_recreate_cause: Option<ActorErrorReason>,
}

impl ActorCellState {
  /// Creates an empty runtime state for a freshly spawned actor cell.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self {
      children_state:          ChildrenContainer::empty(),
      failed:                  FailedInfo::NoFailedInfo,
      watchers:                Vec::new(),
      watching:                Vec::new(),
      terminated_queued:       Vec::new(),
      watch_with_messages:     Vec::new(),
      stashed_messages:        VecDeque::new(),
      timer_handles:           Vec::new(),
      pipe_tasks:              Vec::new(),
      adapter_handles:         Vec::new(),
      adapter_handle_counter:  0,
      pipe_task_counter:       0,
      deferred_recreate_cause: None,
    }
  }
}

impl Default for ActorCellState {
  fn default() -> Self {
    Self::new()
  }
}
