use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use super::schedule_hints::ScheduleHints;

#[cfg(test)]
#[path = "schedule_state_test.rs"]
mod tests;

const FLAG_SCHEDULED: u32 = 1 << 0;
const FLAG_RUNNING: u32 = 1 << 1;
const FLAG_CLOSE_REQUESTED: u32 = 1 << 2;
const FLAG_FINALIZER_OWNED: u32 = 1 << 3;
const FLAG_CLEANUP_DONE: u32 = 1 << 4;
const SUSPEND_SHIFT: u32 = 5;
const SUSPEND_MASK: u32 = !((1 << SUSPEND_SHIFT) - 1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseRequestOutcome {
  CallerOwnsFinalizer,
  RunnerOwnsFinalizer,
  AlreadyRequested,
  AlreadyCleaned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RunFinishOutcome {
  Continue { pending_reschedule: bool },
  FinalizeNow,
  Closed,
}

/// Mailbox-internal schedule state tracking scheduling, running, and suspension.
pub(crate) struct MailboxScheduleState {
  state:           AtomicU32,
  need_reschedule: AtomicBool,
}

impl Default for MailboxScheduleState {
  fn default() -> Self {
    Self::new()
  }
}

impl MailboxScheduleState {
  /// Creates a fresh schedule state in the idle state.
  #[must_use]
  pub(crate) const fn new() -> Self {
    Self { state: AtomicU32::new(0), need_reschedule: AtomicBool::new(false) }
  }

  /// Attempts to mark the mailbox as scheduled. Returns `true` only when the caller
  /// should trigger a new dispatcher execution cycle.
  pub(crate) fn request_schedule(&self, hints: ScheduleHints) -> bool {
    let has_pending = hints.has_system_messages || hints.has_user_messages || hints.backpressure_active;
    debug_assert!(has_pending, "schedule requested without work: {:?}", hints);
    if !has_pending {
      return false;
    }
    if self.is_suspended() && !hints.has_system_messages {
      return false;
    }

    loop {
      let state = self.state.load(Ordering::Acquire);
      if state & (FLAG_CLOSE_REQUESTED | FLAG_CLEANUP_DONE) != 0 {
        return false;
      }
      if state & (FLAG_SCHEDULED | FLAG_RUNNING) != 0 {
        self.need_reschedule.store(true, Ordering::Release);
        return false;
      }
      let desired = state | FLAG_SCHEDULED;
      if self.state.compare_exchange(state, desired, Ordering::AcqRel, Ordering::Acquire).is_ok() {
        return true;
      }
    }
  }

  /// Marks the mailbox as running, clearing any pending scheduled flag.
  pub(crate) fn set_running(&self) {
    loop {
      let state = self.state.load(Ordering::Acquire);
      let desired = (state & !FLAG_SCHEDULED) | FLAG_RUNNING;
      if self.state.compare_exchange(state, desired, Ordering::AcqRel, Ordering::Acquire).is_ok() {
        return;
      }
    }
  }

  /// Clears scheduled/running and returns whether a pending reschedule must occur immediately.
  pub(crate) fn set_idle(&self) -> bool {
    loop {
      let state = self.state.load(Ordering::Acquire);
      let desired = state & !(FLAG_RUNNING | FLAG_SCHEDULED);
      if self.state.compare_exchange(state, desired, Ordering::AcqRel, Ordering::Acquire).is_ok() {
        break;
      }
    }

    self.need_reschedule.swap(false, Ordering::AcqRel)
  }

  /// Increments the suspension counter, preventing user messages from executing.
  pub(crate) fn suspend(&self) {
    self.update_suspend_count(|count| count + 1);
  }

  /// Decrements the suspension counter.
  pub(crate) fn resume(&self) {
    self.update_suspend_count(|count| count.saturating_sub(1));
  }

  /// Returns `true` when user message processing must remain suspended.
  pub(crate) fn is_suspended(&self) -> bool {
    self.current_suspend_count() > 0
  }

  /// Returns `true` while the drain loop is actively running.
  pub(crate) fn is_running(&self) -> bool {
    self.state.load(Ordering::Acquire) & FLAG_RUNNING != 0
  }

  /// Requests terminal close and attempts to elect the cleanup finalizer.
  pub(crate) fn request_close(&self) -> CloseRequestOutcome {
    loop {
      let state = self.state.load(Ordering::Acquire);
      if state & FLAG_CLEANUP_DONE != 0 {
        return CloseRequestOutcome::AlreadyCleaned;
      }
      if state & FLAG_CLOSE_REQUESTED != 0 {
        if state & FLAG_RUNNING != 0 {
          return CloseRequestOutcome::RunnerOwnsFinalizer;
        }
        if state & FLAG_FINALIZER_OWNED != 0 {
          return CloseRequestOutcome::AlreadyRequested;
        }
        let desired = state | FLAG_CLOSE_REQUESTED | FLAG_FINALIZER_OWNED;
        if self.state.compare_exchange(state, desired, Ordering::AcqRel, Ordering::Acquire).is_ok() {
          return CloseRequestOutcome::CallerOwnsFinalizer;
        }
        continue;
      }
      let desired = if state & FLAG_RUNNING != 0 {
        state | FLAG_CLOSE_REQUESTED
      } else {
        state | FLAG_CLOSE_REQUESTED | FLAG_FINALIZER_OWNED
      };
      if self.state.compare_exchange(state, desired, Ordering::AcqRel, Ordering::Acquire).is_ok() {
        return if state & FLAG_RUNNING != 0 {
          CloseRequestOutcome::RunnerOwnsFinalizer
        } else {
          CloseRequestOutcome::CallerOwnsFinalizer
        };
      }
    }
  }

  /// Marks terminal cleanup as complete.
  pub(crate) fn finish_cleanup(&self) {
    loop {
      let state = self.state.load(Ordering::Acquire);
      let desired = state | FLAG_CLEANUP_DONE;
      if self.state.compare_exchange(state, desired, Ordering::AcqRel, Ordering::Acquire).is_ok() {
        return;
      }
    }
  }

  pub(crate) fn is_close_requested(&self) -> bool {
    self.state.load(Ordering::Acquire) & FLAG_CLOSE_REQUESTED != 0
  }

  /// Returns `true` when terminal cleanup has completed.
  pub(crate) fn is_cleanup_done(&self) -> bool {
    self.state.load(Ordering::Acquire) & FLAG_CLEANUP_DONE != 0
  }

  /// Returns `true` when the mailbox has entered terminal close processing.
  pub(crate) fn is_closed(&self) -> bool {
    self.state.load(Ordering::Acquire) & (FLAG_CLOSE_REQUESTED | FLAG_CLEANUP_DONE) != 0
  }

  /// Clears the running flag and reports what the caller should do next.
  pub(crate) fn finish_run(&self) -> RunFinishOutcome {
    loop {
      let state = self.state.load(Ordering::Acquire);
      let mut desired = state & !(FLAG_RUNNING | FLAG_SCHEDULED);
      let close_requested = state & FLAG_CLOSE_REQUESTED != 0;
      let cleanup_done = state & FLAG_CLEANUP_DONE != 0;
      let finalizer_owned = state & FLAG_FINALIZER_OWNED != 0;

      let outcome = if cleanup_done {
        RunFinishOutcome::Closed
      } else if close_requested {
        if finalizer_owned {
          RunFinishOutcome::Closed
        } else {
          desired |= FLAG_FINALIZER_OWNED;
          RunFinishOutcome::FinalizeNow
        }
      } else {
        RunFinishOutcome::Continue { pending_reschedule: false }
      };

      if self.state.compare_exchange(state, desired, Ordering::AcqRel, Ordering::Acquire).is_ok() {
        let pending_reschedule = self.need_reschedule.swap(false, Ordering::AcqRel);
        return match outcome {
          | RunFinishOutcome::Continue { .. } => RunFinishOutcome::Continue { pending_reschedule },
          | other => other,
        };
      }
    }
  }

  fn current_suspend_count(&self) -> u32 {
    (self.state.load(Ordering::Acquire) & SUSPEND_MASK) >> SUSPEND_SHIFT
  }

  fn update_suspend_count(&self, f: impl Fn(u32) -> u32) {
    loop {
      let state = self.state.load(Ordering::Acquire);
      let count = (state & SUSPEND_MASK) >> SUSPEND_SHIFT;
      let new_count = f(count);
      let desired = (state & !SUSPEND_MASK) | (new_count << SUSPEND_SHIFT);
      if self.state.compare_exchange(state, desired, Ordering::AcqRel, Ordering::Acquire).is_ok() {
        break;
      }
    }
  }
}
