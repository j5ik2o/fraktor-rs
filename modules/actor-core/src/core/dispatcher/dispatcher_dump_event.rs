use crate::core::actor_prim::Pid;

/// Snapshot describing dispatcher diagnostics for a single mailbox.
#[derive(Clone, Debug)]
pub struct DispatcherDumpEvent {
  pid:              Pid,
  user_queue_len:   usize,
  system_queue_len: usize,
  running:          bool,
  suspended:        bool,
}

impl DispatcherDumpEvent {
  /// Creates a new dispatcher dump event.
  #[must_use]
  pub const fn new(pid: Pid, user_queue_len: usize, system_queue_len: usize, running: bool, suspended: bool) -> Self {
    Self { pid, user_queue_len, system_queue_len, running, suspended }
  }

  /// Returns the actor pid.
  #[must_use]
  pub const fn pid(&self) -> Pid {
    self.pid
  }

  /// Returns the queued user messages.
  #[must_use]
  pub const fn user_queue_len(&self) -> usize {
    self.user_queue_len
  }

  /// Returns the queued system messages.
  #[must_use]
  pub const fn system_queue_len(&self) -> usize {
    self.system_queue_len
  }

  /// Indicates whether the dispatcher is currently running.
  #[must_use]
  pub const fn is_running(&self) -> bool {
    self.running
  }

  /// Indicates whether the mailbox is suspended.
  #[must_use]
  pub const fn is_suspended(&self) -> bool {
    self.suspended
  }
}
