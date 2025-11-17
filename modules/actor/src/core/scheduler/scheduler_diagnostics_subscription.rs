use alloc::vec::Vec;

use fraktor_utils_rs::core::sync::ArcShared;

use super::{
  SchedulerDiagnosticsEvent,
  diagnostics_registry::{DiagnosticsBuffer, DiagnosticsRegistry},
};

/// Handle returned to diagnostics subscribers for draining events.
pub struct SchedulerDiagnosticsSubscription {
  id:       u64,
  registry: DiagnosticsRegistry,
  buffer:   ArcShared<DiagnosticsBuffer>,
  detached: bool,
}

impl SchedulerDiagnosticsSubscription {
  pub(crate) const fn new(id: u64, registry: DiagnosticsRegistry, buffer: ArcShared<DiagnosticsBuffer>) -> Self {
    Self { id, registry, buffer, detached: false }
  }

  /// Drains and returns all pending diagnostics events.
  #[must_use]
  pub fn drain(&mut self) -> Vec<SchedulerDiagnosticsEvent> {
    self.buffer.drain()
  }
}

impl Drop for SchedulerDiagnosticsSubscription {
  fn drop(&mut self) {
    if !self.detached {
      self.registry.remove(self.id);
      self.detached = true;
    }
  }
}
