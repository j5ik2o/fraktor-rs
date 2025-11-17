use crate::core::scheduler::ExecutionBatch;

/// Kinds of deterministic log entries emitted by the scheduler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeterministicEvent {
  /// Timer registration event.
  Scheduled {
    /// Identifier of the registered handle.
    handle_id:      u64,
    /// Tick when the registration occurred.
    scheduled_tick: u64,
    /// Deadline tick assigned to the timer.
    deadline_tick:  u64,
  },
  /// Timer execution event.
  Fired {
    /// Identifier of the handle that executed.
    handle_id:  u64,
    /// Tick when execution happened.
    fired_tick: u64,
    /// Execution metadata shared with the runnable/message.
    batch:      ExecutionBatch,
  },
  /// Timer cancellation event.
  Cancelled {
    /// Identifier of the cancelled handle.
    handle_id:      u64,
    /// Tick when the cancellation occurred.
    cancelled_tick: u64,
  },
}
