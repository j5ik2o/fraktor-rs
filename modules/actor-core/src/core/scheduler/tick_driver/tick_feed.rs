//! Buffered tick feed shared by drivers and the scheduler executor.

use alloc::collections::VecDeque;
use core::{
  cell::RefCell,
  marker::PhantomData,
  sync::atomic::{AtomicBool, Ordering},
  time::Duration,
};

use critical_section::Mutex;
use fraktor_utils_rs::core::{
  runtime_toolbox::RuntimeToolbox,
  sync::ArcShared,
  time::{SchedulerTickHandle, TimerInstant},
};
use portable_atomic::AtomicU64;

use super::{SchedulerTickHandleOwned, SchedulerTickMetrics, TickDriverKind, TickExecutorSignal};

#[cfg(test)]
mod tests;

/// Shared tick feed handle type.
pub type TickFeedHandle<TB> = ArcShared<TickFeed<TB>>;

/// Maintains buffered ticks plus metrics accounting.
pub struct TickFeed<TB: RuntimeToolbox> {
  _marker:             PhantomData<TB>,
  queue:               Mutex<RefCell<VecDeque<u32>>>,
  capacity:            usize,
  signal:              TickExecutorSignal,
  handle:              SchedulerTickHandleOwned,
  resolution:          Duration,
  enqueued_total:      AtomicU64,
  dropped_total:       AtomicU64,
  window_enqueued:     AtomicU64,
  window_dropped:      AtomicU64,
  last_snapshot_ticks: AtomicU64,
  driver_alive:        AtomicBool,
}

impl<TB: RuntimeToolbox> TickFeed<TB> {
  /// Creates a new feed with the provided capacity.
  #[must_use]
  pub fn new(resolution: Duration, capacity: usize, signal: TickExecutorSignal) -> TickFeedHandle<TB> {
    let bounded_capacity = capacity.max(1);
    let queue = VecDeque::with_capacity(bounded_capacity);
    let feed = Self {
      _marker: PhantomData,
      queue: Mutex::new(RefCell::new(queue)),
      capacity: bounded_capacity,
      signal,
      handle: SchedulerTickHandleOwned::new(),
      resolution,
      enqueued_total: AtomicU64::new(0),
      dropped_total: AtomicU64::new(0),
      window_enqueued: AtomicU64::new(0),
      window_dropped: AtomicU64::new(0),
      last_snapshot_ticks: AtomicU64::new(0),
      driver_alive: AtomicBool::new(false),
    };
    ArcShared::new(feed)
  }

  /// Enqueues ticks supplied by a driver.
  pub fn enqueue(&self, ticks: u32) {
    if ticks == 0 {
      return;
    }
    let pushed = self.try_push(ticks);
    self.finalize_enqueue(pushed, ticks);
  }

  /// Enqueues ticks from interrupt context.
  pub fn enqueue_from_isr(&self, ticks: u32) {
    if ticks == 0 {
      return;
    }
    let pushed = self.try_push(ticks);
    self.finalize_enqueue(pushed, ticks);
  }

  /// Drains buffered ticks, invoking the provided closure for each batch.
  pub(crate) fn drain_pending<F>(&self, mut consumer: F)
  where
    F: FnMut(u32), {
    while let Some(value) = self.pop_front() {
      consumer(value);
    }
  }

  /// Returns the executor signal.
  #[must_use]
  pub fn signal(&self) -> TickExecutorSignal {
    self.signal.clone()
  }

  /// Accesses the underlying tick handle.
  #[must_use]
  pub const fn handle(&self) -> &SchedulerTickHandle<'static> {
    self.handle.handle()
  }

  /// Produces a metrics snapshot.
  pub fn snapshot(&self, now: TimerInstant, driver: TickDriverKind) -> SchedulerTickMetrics {
    let previous = self.last_snapshot_ticks.swap(now.ticks(), Ordering::AcqRel);
    let elapsed_ticks = now.ticks().saturating_sub(previous);
    let window_enqueued = self.window_enqueued.swap(0, Ordering::AcqRel);
    let total_enqueued = self.enqueued_total.load(Ordering::Acquire);
    let total_dropped = self.dropped_total.load(Ordering::Acquire);

    let ticks_per_sec = if elapsed_ticks == 0 {
      window_enqueued.min(u32::MAX as u64) as u32
    } else {
      let elapsed_ns = self.resolution.as_nanos().saturating_mul(u128::from(elapsed_ticks));
      if elapsed_ns == 0 {
        0
      } else {
        let per_sec = u128::from(window_enqueued).saturating_mul(1_000_000_000) / elapsed_ns.max(1);
        per_sec.min(u32::MAX as u128) as u32
      }
    };

    let drift = if elapsed_ticks == 0 {
      None
    } else {
      let expected = i128::from(elapsed_ticks);
      let actual = i128::from(window_enqueued);
      let diff = actual - expected;
      if diff == 0 {
        None
      } else {
        let magnitude = if diff < 0 { diff.saturating_neg() as u128 } else { diff as u128 };
        Some(duration_from_ticks(magnitude, self.resolution))
      }
    };

    self.window_dropped.swap(0, Ordering::AcqRel);

    SchedulerTickMetrics::new(driver, ticks_per_sec, drift, total_enqueued, total_dropped)
  }

  fn try_push(&self, ticks: u32) -> bool {
    critical_section::with(|cs| {
      let mut queue = self.queue.borrow(cs).borrow_mut();
      if queue.len() >= self.capacity {
        false
      } else {
        queue.push_back(ticks);
        true
      }
    })
  }

  fn pop_front(&self) -> Option<u32> {
    critical_section::with(|cs| self.queue.borrow(cs).borrow_mut().pop_front())
  }

  fn record_enqueue(&self, ticks: u32) {
    let value = u64::from(ticks);
    self.enqueued_total.fetch_add(value, Ordering::AcqRel);
    self.window_enqueued.fetch_add(value, Ordering::AcqRel);
  }

  fn record_drop(&self, ticks: u32) {
    let value = u64::from(ticks);
    self.dropped_total.fetch_add(value, Ordering::AcqRel);
    self.window_dropped.fetch_add(value, Ordering::AcqRel);
  }

  fn finalize_enqueue(&self, pushed: bool, ticks: u32) {
    if pushed {
      self.record_driver_activity();
      self.record_enqueue(ticks);
      self.signal.notify();
    } else {
      self.record_drop(ticks);
    }
  }

  fn record_driver_activity(&self) {
    self.driver_alive.store(true, Ordering::Release);
  }

  /// Marks the driver as inactive (used on shutdown).
  pub(crate) fn mark_driver_inactive(&self) {
    self.driver_alive.store(false, Ordering::Release);
  }

  /// Indicates whether the driver recently signaled activity.
  #[must_use]
  pub fn driver_active(&self) -> bool {
    self.driver_alive.load(Ordering::Acquire)
  }
}

fn duration_from_ticks(count: u128, resolution: Duration) -> Duration {
  if count == 0 {
    return Duration::ZERO;
  }
  let nanos = resolution.as_nanos().saturating_mul(count);
  let capped = nanos.min(u64::MAX as u128);
  Duration::from_nanos(capped as u64)
}
