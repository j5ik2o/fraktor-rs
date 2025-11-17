use core::sync::atomic::{AtomicU32, Ordering};

/// Pending tick counter shared between handle and lease.
pub(crate) struct TickState {
  pending: AtomicU32,
}

impl TickState {
  pub(crate) const fn new() -> Self {
    Self { pending: AtomicU32::new(0) }
  }

  pub(crate) fn enqueue(&self, ticks: u32) {
    self.pending.fetch_add(ticks, Ordering::AcqRel);
  }

  pub(crate) fn take(&self) -> u32 {
    self.pending.swap(0, Ordering::AcqRel)
  }
}
