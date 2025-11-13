use core::sync::atomic::{AtomicU8, Ordering};

use crate::scheduler::CancellableState;

/// Shared entry storing the cancellable state.
#[derive(Debug)]
pub struct CancellableEntry {
  state: AtomicU8,
}

impl Default for CancellableEntry {
  fn default() -> Self {
    Self::new()
  }
}

impl CancellableEntry {
  /// Creates a new entry in the pending state.
  #[must_use]
  pub const fn new() -> Self {
    Self { state: AtomicU8::new(CancellableState::Pending as u8) }
  }

  /// Marks the entry as scheduled.
  pub fn mark_scheduled(&self) {
    self.state.store(CancellableState::Scheduled as u8, Ordering::Release);
  }

  /// Attempts to transition into the executing state.
  pub fn try_begin_execute(&self) -> bool {
    self
      .state
      .compare_exchange(
        CancellableState::Scheduled as u8,
        CancellableState::Executing as u8,
        Ordering::AcqRel,
        Ordering::Acquire,
      )
      .is_ok()
  }

  /// Transitions back to the scheduled state (used for periodic jobs).
  pub fn reset_to_scheduled(&self) {
    self.state.store(CancellableState::Scheduled as u8, Ordering::Release);
  }

  /// Attempts to cancel the entry while it is scheduled.
  pub fn try_cancel(&self) -> bool {
    self
      .state
      .compare_exchange(
        CancellableState::Scheduled as u8,
        CancellableState::Cancelled as u8,
        Ordering::AcqRel,
        Ordering::Acquire,
      )
      .is_ok()
  }

  /// Forces the entry into the cancelled state regardless of the current state.
  pub fn force_cancel(&self) {
    self.state.store(CancellableState::Cancelled as u8, Ordering::Release);
  }

  /// Marks execution as completed.
  pub fn mark_completed(&self) {
    self.state.store(CancellableState::Completed as u8, Ordering::Release);
  }

  /// Returns the current state.
  fn current_state(&self) -> CancellableState {
    self.state.load(Ordering::Acquire).into()
  }

  /// Checks whether the entry has been cancelled.
  #[must_use]
  pub fn is_cancelled(&self) -> bool {
    self.current_state() == CancellableState::Cancelled
  }

  /// Checks whether the entry completed execution.
  #[must_use]
  pub fn is_completed(&self) -> bool {
    self.current_state() == CancellableState::Completed
  }
}
