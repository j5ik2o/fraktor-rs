//! Timer wheel storing scheduled entries.

use alloc::{
  collections::{BTreeSet, BinaryHeap},
  vec::Vec,
};
use core::cmp::{Ordering, Reverse};

use super::{TimerEntry, TimerHandleId, TimerInstant, TimerWheelConfig, TimerWheelError};

#[cfg(test)]
mod tests;

/// Timer wheel providing deterministic, FIFO-ordered expiration.
pub struct TimerWheel<P> {
  config:      TimerWheelConfig,
  queue:       BinaryHeap<Reverse<ScheduledEntry<P>>>,
  cancelled:   BTreeSet<TimerHandleId>,
  next_handle: u64,
  sequence:    u64,
  active:      usize,
}

impl<P> TimerWheel<P> {
  /// Creates an empty wheel.
  #[must_use]
  pub const fn new(config: TimerWheelConfig) -> Self {
    Self { config, queue: BinaryHeap::new(), cancelled: BTreeSet::new(), next_handle: 0, sequence: 0, active: 0 }
  }

  /// Returns the number of active (non-cancelled) timers.
  #[must_use]
  pub const fn len(&self) -> usize {
    self.active
  }

  /// Returns `true` when no timers are scheduled.
  #[must_use]
  pub const fn is_empty(&self) -> bool {
    self.active == 0
  }

  /// Schedules a timer entry.
  ///
  /// # Errors
  ///
  /// Returns [`TimerWheelError::ResolutionMismatch`] if the entry's deadline resolution does not
  /// match the wheel's resolution. Returns [`TimerWheelError::CapacityExceeded`] if the wheel is
  /// at capacity.
  pub fn schedule(&mut self, entry: TimerEntry<P>) -> Result<TimerHandleId, TimerWheelError> {
    if entry.deadline().resolution() != self.config.resolution() {
      return Err(TimerWheelError::ResolutionMismatch);
    }
    if self.active >= self.config.slot_count() as usize {
      return Err(TimerWheelError::CapacityExceeded);
    }

    let handle = TimerHandleId::new(self.next_handle);
    self.next_handle = self.next_handle.wrapping_add(1);
    let scheduled = ScheduledEntry::new(handle, entry, self.sequence);
    self.sequence = self.sequence.wrapping_add(1);
    self.queue.push(Reverse(scheduled));
    self.active += 1;
    Ok(handle)
  }

  /// Cancels a scheduled entry if present.
  pub fn cancel(&mut self, handle: TimerHandleId) -> bool {
    if self.cancelled.contains(&handle) {
      return false;
    }
    self.cancelled.insert(handle);
    if self.active > 0 {
      self.active -= 1;
    }
    true
  }

  /// Drains all expired entries up to `now`.
  ///
  /// # Panics
  ///
  /// This function should not panic under normal conditions. The internal queue invariant
  /// guarantees that if `peek()` returns `Some`, then `pop()` will also return `Some`.
  pub fn collect_expired(&mut self, now: TimerInstant) -> Vec<TimerEntry<P>> {
    let mut expired = Vec::new();
    while let Some(Reverse(scheduled)) = self.queue.peek() {
      if scheduled.deadline_tick > now.ticks() {
        break;
      }
      // SAFETY: peek() returned Some, so pop() must also return Some.
      // unwrap_or_else を使用してロジックエラーの場合はスキップする
      let Some(Reverse(scheduled)) = self.queue.pop() else {
        continue;
      };
      if self.cancelled.remove(&scheduled.handle_id) {
        continue;
      }
      if self.active > 0 {
        self.active -= 1;
      }
      expired.push(scheduled.entry);
    }
    expired
  }
}

struct ScheduledEntry<P> {
  handle_id:     TimerHandleId,
  deadline_tick: u64,
  sequence:      u64,
  entry:         TimerEntry<P>,
}

impl<P> ScheduledEntry<P> {
  const fn new(handle_id: TimerHandleId, entry: TimerEntry<P>, sequence: u64) -> Self {
    Self { handle_id, deadline_tick: entry.deadline().ticks(), sequence, entry }
  }
}

impl<P> PartialEq for ScheduledEntry<P> {
  fn eq(&self, other: &Self) -> bool {
    self.deadline_tick == other.deadline_tick && self.sequence == other.sequence
  }
}

impl<P> Eq for ScheduledEntry<P> {}

impl<P> PartialOrd for ScheduledEntry<P> {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl<P> Ord for ScheduledEntry<P> {
  fn cmp(&self, other: &Self) -> Ordering {
    self.deadline_tick.cmp(&other.deadline_tick).then_with(|| self.sequence.cmp(&other.sequence))
  }
}
