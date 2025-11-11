use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

mod schedule_hints;

pub use schedule_hints::ScheduleHints;

#[cfg(test)]
mod tests;

const FLAG_SCHEDULED: u32 = 1 << 0;
const FLAG_RUNNING: u32 = 1 << 1;
const FLAG_CLOSED: u32 = 1 << 2;
const SUSPEND_SHIFT: u32 = 3;
const SUSPEND_MASK: u32 = !((1 << SUSPEND_SHIFT) - 1);

/// Mailbox-internal state machine that tracks scheduling, running, and suspension.
pub struct MailboxStateEngine {
  state:           AtomicU32,
  need_reschedule: AtomicBool,
}

impl MailboxStateEngine {
  /// Creates a fresh state engine in the idle state.
  pub const fn new() -> Self {
    Self { state: AtomicU32::new(0), need_reschedule: AtomicBool::new(false) }
  }

  /// Attempts to mark the mailbox as scheduled. Returns `true` only when the caller
  /// should trigger a new dispatcher execution cycle.
  pub fn request_schedule(&self, hints: ScheduleHints) -> bool {
    let effective = self.has_effective_work(hints);
    debug_assert!(effective, "schedule requested without work: {:?}", hints);
    if !effective {
      return false;
    }

    loop {
      let state = self.state.load(Ordering::Acquire);
      if state & FLAG_CLOSED != 0 {
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
  pub fn set_running(&self) {
    loop {
      let state = self.state.load(Ordering::Acquire);
      let desired = (state & !FLAG_SCHEDULED) | FLAG_RUNNING;
      if self.state.compare_exchange(state, desired, Ordering::AcqRel, Ordering::Acquire).is_ok() {
        return;
      }
    }
  }

  /// Clears the running flag. Returns `true` if mailbox should re-schedule immediately.
  pub fn set_idle(&self) -> bool {
    loop {
      let state = self.state.load(Ordering::Acquire);
      let desired = state & !FLAG_RUNNING;
      if self.state.compare_exchange(state, desired, Ordering::AcqRel, Ordering::Acquire).is_ok() {
        break;
      }
    }

    self.need_reschedule.swap(false, Ordering::AcqRel)
  }

  /// Increments the suspension counter, preventing user messages from executing.
  pub fn suspend(&self) {
    self.update_suspend_count(|count| count + 1);
  }

  /// Decrements the suspension counter.
  pub fn resume(&self) {
    self.update_suspend_count(|count| count.saturating_sub(1));
  }

  /// Returns `true` when user message processing must remain suspended.
  pub fn is_suspended(&self) -> bool {
    self.current_suspend_count() > 0
  }

  fn has_effective_work(&self, hints: ScheduleHints) -> bool {
    hints.has_system_messages || ((hints.has_user_messages || hints.backpressure_active) && !self.is_suspended())
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
