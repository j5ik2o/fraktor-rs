use fraktor_utils_core_rs::sync::{DefaultMutex, SharedAccess, SharedLock};

#[cfg(test)]
#[path = "stream_island_drive_gate_test.rs"]
mod tests;

/// Coalescing gate for `Drive` commands targeting one stream island actor.
pub(crate) struct StreamIslandDriveGate {
  pending: SharedLock<bool>,
}

impl Clone for StreamIslandDriveGate {
  fn clone(&self) -> Self {
    Self { pending: self.pending.clone() }
  }
}

impl StreamIslandDriveGate {
  pub(crate) fn new() -> Self {
    Self { pending: SharedLock::new_with_driver::<DefaultMutex<_>>(false) }
  }

  #[must_use]
  pub(crate) fn try_mark_pending(&self) -> bool {
    self.pending.with_write(|pending| {
      if *pending {
        return false;
      }
      *pending = true;
      true
    })
  }

  pub(crate) fn mark_idle(&self) {
    self.pending.with_write(|pending| {
      *pending = false;
    });
  }
}
