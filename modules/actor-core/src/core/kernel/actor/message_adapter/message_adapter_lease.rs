//! Opaque release handle for message adapters.

use super::AdapterRefHandleId;
use crate::core::kernel::{actor::Pid, system::state::SystemStateShared};

/// Opaque release handle for a message adapter registration.
pub struct MessageAdapterLease {
  pid:       Pid,
  handle_id: AdapterRefHandleId,
  system:    SystemStateShared,
  released:  bool,
}

impl MessageAdapterLease {
  #[must_use]
  pub(crate) const fn new(pid: Pid, handle_id: AdapterRefHandleId, system: SystemStateShared) -> Self {
    Self { pid, handle_id, system, released: false }
  }

  /// Releases the adapter registration and stops associated adapter senders.
  pub fn release(mut self) {
    self.release_inner();
  }

  fn release_inner(&mut self) {
    if self.released {
      return;
    }
    self.released = true;
    if let Some(cell) = self.system.cell(&self.pid) {
      cell.remove_adapter_handle(self.handle_id);
    }
  }
}

impl Drop for MessageAdapterLease {
  fn drop(&mut self) {
    self.release_inner();
  }
}
